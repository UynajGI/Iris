"""Measure actual compiled RAW development and oversize isolation without ExifTool."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/validation-python"))
from validation_support import Daemon


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, default=ROOT / "target/release/iris-daemon.exe")
    parser.add_argument("--models", type=Path, default=ROOT / "models")
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output must be new")
    photos = args.output / "photos"; photos.mkdir(parents=True)
    models = args.output / "models"; models.mkdir()
    for source in args.models.iterdir():
        if source.is_file():
            shutil.copy2(source, models / source.name)
    sources = [ROOT / "artifacts/public-corpus/native" / n for n in ["iphone-6s.dng", "sony-a7r.arw"]]
    hashes = {str(p): digest(p) for p in sources}
    for source in sources:
        shutil.copy2(source, photos / source.name)
    report = {"ok": False, "daemon_sha256": digest(args.daemon), "exiftool_present": False,
        "corpus_manifest": str(ROOT / "artifacts/public-corpus/native/manifest.json")}
    try:
        with Daemon(args.daemon, models, args.output / "run", workers=1) as daemon:
            project = daemon.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
            report["scan"] = daemon.job(project, "scan")
            report["analysis"] = daemon.job(project, "analyze", expected="failed")
            listing = daemon.request("GET", f"/projects/{project}/photos")
            dng = next(p for p in listing if p["filename"].endswith("dng"))
            arw = next(p for p in listing if p["filename"].endswith("arw"))
            assert dng["analysis_status"] == "current" and dng["analysis"]["preview_source"] == "raw_developed"
            assert arw["analysis"] is None
            assert report["analysis"]["result"]["analyzed"] == 1 and report["analysis"]["result"]["failed"] == 1
            assert any("24" in error or "develop" in error.lower() for error in report["analysis"]["errors"])
            report["memory"] = daemon.memory
            report["ok"] = True
    finally:
        report["source_hashes_preserved"] = all(digest(Path(p)) == sha for p, sha in hashes.items())
        (args.output / "report.json").write_text(json.dumps(report, indent=2), "utf-8")
    assert report["source_hashes_preserved"]
    print(json.dumps(report))


if __name__ == "__main__":
    main()
