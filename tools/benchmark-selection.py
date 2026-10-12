"""Repeatable local selection benchmark; an explicit public-fixture manifest is required.

Server response timings are not browser paint timings. Sources are only read and
copied into an isolated workspace; all caches, marks and databases are disposable.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import statistics
import subprocess
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.file_digest(path.open("rb"), "sha256").hexdigest()


def summary(values):
    ordered = sorted(values)
    return {
        "n": len(values),
        "median_ms": statistics.median(values),
        "p95_ms": ordered[math.ceil(len(ordered) * 0.95) - 1],
        "max_ms": ordered[-1],
    }


class Client:
    def __init__(self, session):
        self.session = session

    def request(self, method, path, body=None, binary=False):
        req = urllib.request.Request(
            self.session["base_url"] + "/api/v1" + path,
            data=None if body is None else json.dumps(body).encode(),
            method=method,
            headers={
                "Authorization": "Bearer " + self.session["token"],
                "Content-Type": "application/json",
            },
        )
        with urllib.request.urlopen(req, timeout=180) as response:
            data = response.read()
        return data if binary else json.loads(data)

    def timed(self, method, path, body=None, binary=False):
        start = time.perf_counter()
        value = self.request(method, path, body, binary)
        return value, (time.perf_counter() - start) * 1000

    def wait(self, project, kind, observe=False):
        start = time.perf_counter()
        self.request("POST", f"/projects/{project}/{kind}")
        first = None
        first_page = None
        quality = None
        samples = []
        while time.perf_counter() - start < 1800:
            progress = self.request("GET", f"/projects/{project}/progress")
            if observe:
                photos = self.request("GET", f"/projects/{project}/photos?limit=48")
                if photos and first is None:
                    first = (time.perf_counter() - start) * 1000
                if len(photos) >= 48 and first_page is None:
                    first_page = (time.perf_counter() - start) * 1000
                if kind == "analyze" and photos:
                    _, elapsed = self.timed(
                        "POST",
                        f"/projects/{project}/marks",
                        {"photo_ids": [photos[0]["id"]], "rating": 3},
                    )
                    samples.append(elapsed)
            if progress.get("quality_ready") and quality is None:
                quality = (time.perf_counter() - start) * 1000
            if progress["state"] in ["completed", "cancelled", "failed"]:
                assert progress["state"] == "completed" and not progress["errors"], progress
                return {
                    "wall_ms": (time.perf_counter() - start) * 1000,
                    "first_list_ms": first,
                    "first_48_ms": first_page,
                    "quality_ready_ms": quality,
                    "progress": progress,
                    "mark_latency": summary(samples) if samples else None,
                }
            time.sleep(0.05 if kind == "scan" else 0.15)
        raise TimeoutError(kind)


def measure(client, args, report, photos, count):
    project = client.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
    report["scan_fresh"] = client.wait(project, "scan", True)
    listing = client.request("GET", f"/projects/{project}/photos")
    assert len(listing) == count
    ids = [photo["id"] for photo in listing[:60]]

    def previews(ids):
        return [client.timed("GET", f"/photos/{id}/preview", binary=True)[1] for id in ids]

    report["preview_fresh"] = summary(previews(ids))
    report["preview_cached"] = summary(previews(ids))
    # Request a thumbnail backlog while repeatedly opening a foreground preview.
    with ThreadPoolExecutor(max_workers=12) as pool:
        backlog = [
            pool.submit(client.timed, "GET", f"/photos/{p['id']}/thumb", None, True)
            for p in listing[60:120]
        ]
        report["preview_under_thumbnail_load"] = summary(previews(ids[:20]))
        for future in backlog:
            future.result()
    report["scan_cached"] = client.wait(project, "scan")
    if args.dino or args.device is not None:
        settings = client.request("GET", f"/settings?project_id={project}")
        if args.dino:
            settings.update(
                embedding_provider="dinov3_vits16",
                embedding_model_sha256="f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c",
                semantic_similarity_threshold=0.85,
            )
        if args.device is not None:
            settings.update(execution_provider="directml", directml_device_id=args.device)
        client.request("PUT", f"/settings?project_id={project}", settings)
    report["analysis_fresh"] = client.wait(project, "analyze", True)
    report["analysis_photos_per_second"] = count / (report["analysis_fresh"]["wall_ms"] / 1000)
    report["analysis_cached"] = client.wait(project, "analyze")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--dino", action="store_true")
    parser.add_argument("--device", type=int)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    records = manifest.get("images") if isinstance(manifest, dict) else manifest
    records = [
        record
        for record in records
        if "PPR10" in record.get("dataset", "")
        and Path(record["path"]).suffix.lower() in [".jpg", ".jpeg"]
    ]
    assert len(records) >= 500, "Use at least 500 manifest-identified public PPR10K portraits"
    assert not args.work.exists(), "Use a fresh work directory for an independent application cache"
    args.work.mkdir(parents=True)
    photos = args.work / "photos"
    photos.mkdir()
    for record in records:
        source = (args.manifest.parent / record["path"]).resolve()
        assert source.is_relative_to(args.manifest.parent.resolve()), (
            "manifest path escapes dataset"
        )
        assert sha(source) == record["sha256"], str(source)
        target = photos / record["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    report = {
        "count": len(records),
        "logical_cpus": os.cpu_count(),
        "binary_sha256": sha(args.binary),
        "manifest_sha256": sha(args.manifest),
        "public_data_only": True,
        "browser_paint_measured": False,
        "filesystem_cache": "uncontrolled; application caches and database fresh",
        "dino": args.dino,
        "device": args.device,
    }
    with (args.work / "daemon.log").open("w", encoding="utf-8") as log:
        process = subprocess.Popen(
            [
                str(args.binary.resolve()),
                "--data-dir",
                str((args.work / "data").resolve()),
                "--model-dir",
                str(ROOT / "models"),
            ],
            cwd=ROOT,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=log,
            text=True,
            env=os.environ | {"IRIS_DAEMON_WORKER_PATH": str(args.binary.resolve())},
        )
        try:
            start = time.perf_counter()
            client = Client(json.loads(process.stdout.readline()))
            report["daemon_start_ms"] = (time.perf_counter() - start) * 1000
            measure(client, args, report, photos, len(records))
            report["sources_unchanged"] = all(
                sha(args.manifest.parent / record["path"]) == record["sha256"] for record in records
            )
            report["ok"] = report["sources_unchanged"]
        finally:
            process.stdin.write("shutdown\n")
            process.stdin.flush()
            try:
                process.wait(timeout=30)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(
                json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
            )
    print(
        json.dumps(
            {
                key: report.get(key)
                for key in [
                    "ok",
                    "count",
                    "analysis_photos_per_second",
                    "preview_fresh",
                    "preview_cached",
                ]
            },
            ensure_ascii=False,
        ),
        flush=True,
    )


if __name__ == "__main__":
    main()
