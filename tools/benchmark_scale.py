"""Measure sustained JPG workload using repeated hard links to the private sample.

This measures scale, not accuracy or a diverse 3000-photo collection. It never
invokes decisions, export, quarantine, or any source-writing endpoint.
"""
from __future__ import annotations

import argparse
from contextlib import closing
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.request


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_repeated_predictions(database: sqlite3.Connection, expected_paths: dict[str, int]) -> dict:
    """Check all repeated inputs retain identical full non-timing predictions.

    Run after timed phases. This detects missing/extra rows and divergent worker
    results; it does not supply independent truth or validate model accuracy.
    """
    seen = set()
    by_source = {}
    for path, data in database.execute(
        "SELECT p.path,a.data FROM photos p JOIN analyses a ON a.photo_id=p.id"
    ):
        path = path.replace("\\", "/")
        assert path in expected_paths and path not in seen, f"unexpected or duplicate analysis path: {path}"
        seen.add(path)
        analysis = json.loads(data)
        analysis.pop("elapsed_ms", None)
        encoded = json.dumps(analysis, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()
        checksum = hashlib.sha256(encoded).hexdigest()
        source = expected_paths[path]
        if source in by_source:
            assert by_source[source] == checksum, f"repeated source prediction differs: {path}"
        else:
            by_source[source] = checksum
    assert seen == expected_paths.keys(), "some expected repeated inputs have no analysis"
    return {"checked_analyses": len(seen), "unique_sources": len(by_source),
            "all_repeated_non_timing_predictions_equal": True,
            "per_source_predictions_sha256": {str(k): v for k, v in sorted(by_source.items())},
            "excluded_fields": ["elapsed_ms"], "performed_after_timed_phases": True}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=Path("test-photos"))
    parser.add_argument("--daemon", type=Path, default=Path("target/release/iris-daemon.exe"))
    parser.add_argument("--models", type=Path, default=Path("models"))
    parser.add_argument("--repeat", type=int, default=30)
    parser.add_argument("--timeout", type=float, default=1800)
    parser.add_argument("--worker-limit", type=int, choices=range(1, 17), help="Daemon process concurrency policy; omitted preserves its default")
    parser.add_argument("--output", type=Path, default=Path("artifacts/scale-3000.json"))
    args = parser.parse_args()
    if not 1 <= args.repeat <= 100 or args.timeout <= 0:
        parser.error("repeat must be 1..100 and timeout must be positive")
    if args.output.exists():
        parser.error("output already exists; choose a new report path")
    fixtures = args.fixtures.resolve(strict=True)
    manifest = json.loads((fixtures / "manifest.json").read_text("utf-8"))
    entries = manifest["files"]
    assert len(entries) == 100 and manifest["count"] == 100
    sources = []
    for entry in entries:
        source = (fixtures / entry["path"]).resolve(strict=True)
        assert source.is_relative_to(fixtures) and source.suffix.lower() in {".jpg", ".jpeg"}
        assert digest(source) == entry["sha256"]
        sources.append(source)
    count = len(sources) * args.repeat
    report = {
        "workload": "repeated hard links to 100 private JPGs; no new scene diversity",
        "unique_source_photos": len(sources), "repeats": args.repeat, "photo_count": count,
        "accuracy_evaluated": False, "cpu_logical_count": os.cpu_count(),
        "platform": platform.platform(), "daemon_sha256": digest(args.daemon),
        "phases": {}, "complete": False, "requested_worker_limit": args.worker_limit,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # Use the fixture volume so hard links never require copying 30x the bytes.
    with tempfile.TemporaryDirectory(prefix="iris-scale-", dir=fixtures.parent / "artifacts") as temporary:
        work = Path(temporary).resolve()
        assert work.is_relative_to((fixtures.parent / "artifacts").resolve())
        photos = work / "photos"
        photos.mkdir()
        expected_paths = {}
        for repeat in range(args.repeat):
            batch = photos / f"batch-{repeat:03d}"
            batch.mkdir()
            for index, source in enumerate(sources):
                target = batch / f"{index:03d}-{source.name}"
                os.link(source, target)
                expected_paths[target.relative_to(photos).as_posix()] = index
        process = None
        try:
            with (work / "daemon-stderr.log").open("w", encoding="utf-8") as stderr:
                command = [str(args.daemon.resolve()), "--data-dir", str(work / "data"),
                           "--model-dir", str(args.models.resolve())]
                if args.worker_limit is not None:
                    command += ["--worker-limit", str(args.worker_limit)]
                process = subprocess.Popen(
                    command,
                    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr, text=True,
                )
                handshake_queue = queue.Queue()
                threading.Thread(target=lambda: handshake_queue.put(process.stdout.readline()), daemon=True).start()
                handshake = json.loads(handshake_queue.get(timeout=30))

                def request(method, path, body=None):
                    payload = None if body is None else json.dumps(body).encode()
                    req = urllib.request.Request(
                        handshake["base_url"] + "/api/v1" + path, data=payload, method=method,
                        headers={"Authorization": "Bearer " + handshake["token"], "Content-Type": "application/json"},
                    )
                    with urllib.request.urlopen(req, timeout=60) as response:
                        return json.load(response)

                project = request("POST", "/projects", {"root": str(photos)})["id"]

                def job(label, endpoint):
                    start = time.monotonic()
                    request("POST", f"/projects/{project}/{endpoint}")
                    while time.monotonic() - start < args.timeout:
                        progress = request("GET", f"/projects/{project}/progress")
                        if progress["state"] in {"completed", "failed", "cancelled"}:
                            report["phases"][label] = {"seconds": round(time.monotonic() - start, 3), **progress}
                            assert progress["state"] == "completed" and not progress["errors"], progress
                            return progress
                        time.sleep(0.5)
                    raise TimeoutError(f"{label} exceeded {args.timeout} seconds")

                job("first_scan", "scan")
                first = job("first_analysis", "analyze")
                assert first["result"]["analyzed"] == count and first["result"]["failed"] == 0
                job("incremental_scan", "scan")
                cached = job("cached_analysis", "analyze")
                assert cached["result"]["reused"] == count and cached["result"]["workers"] == 0
                state = request("GET", f"/projects/{project}")
                assert state["pending_analysis"] == 0 and not state["groups_dirty"]
                uri = (work / "data/library.sqlite3").as_uri() + "?mode=ro"
                with closing(sqlite3.connect(uri, uri=True)) as database:
                    assert database.execute("pragma quick_check").fetchone() == ("ok",)
                    assert database.execute("select count(*) from analyses").fetchone()[0] == count
                    report["prediction_consistency"] = verify_repeated_predictions(database, expected_paths)
                report["project_ready"] = True
                report["first_scan_and_analysis_seconds"] = round(sum(report["phases"][p]["seconds"] for p in ["first_scan", "first_analysis"]), 3)
                report["incremental_scan_and_analysis_seconds"] = round(sum(report["phases"][p]["seconds"] for p in ["incremental_scan", "cached_analysis"]), 3)
                report["analysis_photos_per_second"] = round(count / report["phases"]["first_analysis"]["seconds"], 3)
                report["complete"] = True
        except Exception as error:
            report["error"] = str(error)
            raise
        finally:
            if process is not None:
                if process.poll() is None:
                    try:
                        process.stdin.write("shutdown\n")
                        process.stdin.flush()
                        process.wait(timeout=15)
                    except (BrokenPipeError, subprocess.TimeoutExpired):
                        process.kill()
                        process.wait(timeout=15)
                process.stdin.close()
                process.stdout.close()
            report["source_hashes_preserved"] = all(digest(source) == entry["sha256"] for source, entry in zip(sources, entries))
            args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2), "utf-8")
    assert report["source_hashes_preserved"]
    print(json.dumps({k: report[k] for k in ["photo_count", "first_scan_and_analysis_seconds", "incremental_scan_and_analysis_seconds", "analysis_photos_per_second", "source_hashes_preserved"]}))


if __name__ == "__main__":
    main()
