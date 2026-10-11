"""Exercise downloaded public research fixtures through the real local daemon."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "reference/test-data"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Client:
    def __init__(self, session: dict):
        self.session = session

    def request(self, method: str, path: str, body=None, binary=False):
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

    def job(self, project: int, kind: str) -> dict:
        self.request("POST", f"/projects/{project}/{kind}")
        started = time.monotonic()
        last_log = started
        while time.monotonic() - started < 1800:
            progress = self.request("GET", f"/projects/{project}/progress")
            if progress["state"] in {"completed", "failed", "cancelled"}:
                return progress
            if time.monotonic() - last_log > 30:
                print(f"{kind}: {progress['completed']}/{progress['total']}", flush=True)
                last_log = time.monotonic()
            time.sleep(0.2)
        raise TimeoutError(kind)


def preview(client: Client, photo: dict) -> dict:
    result = {"name": photo["filename"], "format": photo["format"]}
    try:
        payload = client.request("GET", f"/photos/{photo['id']}/preview", binary=True)
        assert payload.startswith(b"\xff\xd8"), "Expected JPEG preview"
        result["preview_bytes"] = len(payload)
    except Exception as error:
        result["error"] = str(error)
    return result


def pipeline_passed(
    jobs: list[dict], media: list[dict], analyzed: int, scanned: int, expected: int
) -> bool:
    return (
        scanned == analyzed == expected
        and all(job["state"] == "completed" and not job["errors"] for job in jobs)
        and not any("error" in item for item in media)
        and (jobs[-1].get("result") or {}).get("reused") == expected
    )


def exercise(client: Client, photos: Path, count: int) -> dict:
    project = client.request("POST", "/projects", {"root": str(photos)})["id"]
    scan = client.job(project, "scan")
    listing = client.request("GET", f"/projects/{project}/photos")
    print(f"Scanned {len(listing)}/{count}; scan errors: {len(scan['errors'])}", flush=True)
    media = []
    with ThreadPoolExecutor(max_workers=4) as pool:
        for item in pool.map(lambda photo: preview(client, photo), listing):
            media.append(item)
            if len(media) % 50 == 0:
                print(f"Checked previews {len(media)}/{count}", flush=True)
    analysis = client.job(project, "analyze")
    listing = client.request("GET", f"/projects/{project}/photos")
    current = [
        p
        for p in listing
        if p["analysis_status"] == "current" and (p["analysis"] or {}).get("selection_evidence")
    ]
    print(f"Analyzed {len(current)} images; checking incremental reuse", flush=True)
    repeat = client.job(project, "analyze")
    return {
        "ok": pipeline_passed([scan, analysis, repeat], media, len(current), len(listing), count),
        "scanned": len(listing),
        "analyzed": len(current),
        "previews_succeeded": sum("error" not in item for item in media),
        "scan": scan,
        "analysis": analysis,
        "repeat": repeat,
        "media": media,
        "faces_detected_in": sum(bool(p["analysis"]["faces"]) for p in current),
        "preview_sources": sorted(
            {p["analysis"].get("preview_source") or "unknown" for p in current}
        ),
        "groups_returned": len(client.request("GET", f"/projects/{project}/groups")),
    }


def parse_inputs() -> tuple[argparse.Namespace, list[dict]]:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", help="Filter the manifest by dataset name substring")
    parser.add_argument("--limit", type=int, help="Optional positive diagnostic subset size")
    parser.add_argument(
        "--daemon",
        type=Path,
        default=ROOT / "target/debug/iris-daemon.exe",
        help="Daemon executable; prefer a current release build for a large RAW corpus",
    )
    parser.add_argument(
        "--output", type=Path, default=ROOT / "artifacts/public-fixtures-verification.json"
    )
    args = parser.parse_args()
    manifest = json.loads((FIXTURES / "manifest.json").read_text(encoding="utf-8"))
    if args.dataset:
        manifest = [item for item in manifest if args.dataset.lower() in item["dataset"].lower()]
    if args.limit is not None:
        if args.limit < 1:
            parser.error("--limit must be positive")
        manifest = manifest[: args.limit]
    if not manifest:
        parser.error("No downloaded fixtures match")
    return args, manifest


def main() -> None:
    args, manifest = parse_inputs()
    report = {
        "ok": False,
        "accuracy_validated": False,
        "personal_photos_used": False,
        "count": len(manifest),
        "daemon": str(args.daemon.resolve()),
    }
    # Keep currently unsupported formats as explicit negative import controls.
    unsupported = [
        item for item in manifest if Path(item["path"]).suffix.lower() in {".iiq", ".3fr"}
    ]
    report["unsupported_controls"] = [item["path"] for item in unsupported]
    try:
        with tempfile.TemporaryDirectory(prefix="iris-public-fixtures-") as temporary:
            work = Path(temporary)
            photos = work / "photos"
            photos.mkdir()
            for index, item in enumerate(manifest):
                source = FIXTURES / item["path"]
                assert digest(source) == item["sha256"], item["path"]
                shutil.copyfile(source, photos / f"{index:03d}-{source.name}")
            with (work / "daemon.log").open("w", encoding="utf-8") as log:
                process = subprocess.Popen(
                    [
                        str(args.daemon.resolve()),
                        "--data-dir",
                        str(work / "data"),
                        "--model-dir",
                        str(ROOT / "models"),
                        "--port",
                        "0",
                    ],
                    stdin=subprocess.PIPE,
                    stdout=subprocess.PIPE,
                    stderr=log,
                    text=True,
                )
                try:
                    client = Client(json.loads(process.stdout.readline()))
                    report.update(exercise(client, photos, len(manifest) - len(unsupported)))
                finally:
                    process.stdin.write("shutdown\n")
                    process.stdin.flush()
                    process.stdin.close()
                    try:
                        process.wait(timeout=20)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()
            assert all(digest(FIXTURES / item["path"]) == item["sha256"] for item in manifest)
            report.update(completed=True, sources_unchanged=True)
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        output = args.output
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        print(
            json.dumps(
                {key: report[key] for key in ("ok", "count", "accuracy_validated")},
                ensure_ascii=False,
            )
        )
    if not report["ok"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
