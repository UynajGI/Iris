"""Public native formats, generated portrait variants, orientation/size/error isolation."""
import argparse
from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import shutil
import struct
import sys
import zlib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "artifacts/validation-python"))
import numpy as np
from PIL import Image, ImageOps, ImageDraw
import pillow_heif
from validation_support import Daemon
pillow_heif.register_heif_opener()


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=ROOT / "artifacts/public-corpus")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--daemon", type=Path, default=ROOT / "target/release/iris-daemon.exe")
    parser.add_argument("--models", type=Path, default=ROOT / "models")
    args = parser.parse_args()
    if args.output.exists():
        parser.error("output must be new")
    photos = args.output / "photos"
    photos.mkdir(parents=True)
    entries = []
    source_hashes = {}
    native_manifest = json.loads((args.corpus / "native/manifest.json").read_text("utf-8"))
    native_entries = {e["path"]: e for e in native_manifest["files"]}
    for source in (args.corpus / "native").iterdir():
        if source.suffix.lower() not in {".heic", ".heif", ".cr2", ".arw", ".nef", ".dng"}:
            continue
        source_hashes[str(source)] = digest(source)
        assert source_hashes[str(source)] == native_entries[source.name]["sha256"]
        shutil.copy2(source, photos / source.name)
        entries.append({"path": source.name, "kind": "native", "source": str(source), "manifest": str(args.corpus / "native/manifest.json")})
    portrait_root = args.corpus / "portraits-960"
    portrait_manifest = json.loads((portrait_root / "manifest.json").read_text("utf-8"))
    portrait_entries = {e["path"]: e for e in portrait_manifest["files"]}
    available = sorted(portrait_root / name for name in portrait_entries)
    assert len(available) >= 3, "need at least three downloaded public portraits"
    for index, source in enumerate(available):
        source_hashes[str(source)] = digest(source)
        assert source_hashes[str(source)] == portrait_entries[source.name]["sha256"]
        with Image.open(source) as opened:
            im = ImageOps.exif_transpose(opened).convert("RGB")
        for extension in ["jpg", "png", "webp", "heic"]:
            name = f"portrait-{index:02}.{extension}"
            im.save(photos / name, quality=90)
            entries.append({"path": name, "kind": "public_portrait_conversion", "source": str(source), "provenance": portrait_entries[source.name], "modifications": "EXIF transpose, RGB conversion and format encoding"})
    base = Image.new("RGB", (640, 400), "red")
    draw = ImageDraw.Draw(base)
    draw.rectangle((320, 0, 639, 199), fill="green")
    draw.rectangle((0, 200, 319, 399), fill="blue")
    draw.rectangle((320, 200, 639, 399), fill="yellow")
    expected = {}
    for extension in ["png", "webp"]:
        for orientation in range(1, 9):
            exif = Image.Exif(); exif[274] = orientation
            name = f"orientation-{orientation}.{extension}"
            base.save(photos / name, exif=exif, lossless=True)
            with Image.open(photos / name) as opened:
                expected[name] = ImageOps.exif_transpose(opened).convert("RGB")
            entries.append({"path": name, "kind": "synthetic_orientation", "orientation": orientation})
    for extension in ["png", "webp"]:
        name = f"large-24mp.{extension}"
        base.resize((6000, 4000)).save(photos / name, lossless=True)
        entries.append({"path": name, "kind": "synthetic_24mp"})
    base.resize((4000, 2500)).save(photos / "embedded-thumbnail.heic", thumbnails=[1280], quality=80)
    entries.append({"path": "embedded-thumbnail.heic", "kind": "synthetic_heic_thumbnail", "primary_dimensions": [4000, 2500], "thumbnail_edge": 1280})
    for extension in ["jpg", "png", "webp", "heic", "nef"]:
        (photos / f"broken.{extension}").write_bytes(b"deliberately corrupt test input")
    oversized = bytearray((photos / "orientation-1.png").read_bytes())
    oversized[16:24] = struct.pack(">II", 10000, 6100)
    oversized[29:33] = struct.pack(">I", zlib.crc32(oversized[12:29]))
    (photos / "oversized.png").write_bytes(oversized)
    original = {p.name: digest(p) for p in photos.iterdir()}
    (args.output / "fixture-manifest.json").write_text(json.dumps({"files": entries, "hashes": original, "intentional_failures": 6}, indent=2), "utf-8")
    report = {"ok": False, "daemon_sha256": digest(args.daemon), "native_files": len(native_entries), "unique_public_portrait_sources": len(available),
              "derived_formats_are_not_independent_scenes": True, "accuracy_evaluated": False, "orientation_checks": {}}
    try:
        with Daemon(args.daemon, args.models, args.output / "run", workers=2) as daemon:
            project = daemon.request("POST", "/projects", {"root": str(photos.resolve())})["id"]
            report["scan"] = daemon.job(project, "scan", expected="failed")
            listing = daemon.request("GET", f"/projects/{project}/photos")
            assert len(listing) == len(entries), report["scan"]
            assert len(report["scan"]["errors"]) == 6, report["scan"]
            mime = {"jpeg": "image/jpeg", "png": "image/png", "webp": "image/webp", "heic": "image/heic", "raw": "application/octet-stream"}
            for photo in listing:
                name = photo["filename"]
                content_type, data = daemon.request("GET", f"/photos/{photo['id']}/original", raw=True)
                assert content_type == mime[photo["format"]] and hashlib.sha256(data).hexdigest() == original[name]
                for route in ["thumb", "preview"]:
                    content_type, data = daemon.request("GET", f"/photos/{photo['id']}/{route}", raw=True)
                    decoded = Image.open(io.BytesIO(data)).convert("RGB")
                    assert content_type == "image/jpeg" and max(decoded.size) <= (320 if route == "thumb" else 2560)
                    if name in expected:
                        reference = expected[name].resize(decoded.size, Image.Resampling.BILINEAR)
                        error = float(np.abs(np.asarray(decoded).astype(float) - np.asarray(reference).astype(float)).mean())
                        assert error < 6, (name, route, error)
                        assert (decoded.width > decoded.height) == (expected[name].width > expected[name].height)
                        report["orientation_checks"][name + "/" + route] = error
            report["analysis"] = daemon.job(project, "analyze")
            assert report["analysis"]["result"]["analyzed"] == len(entries)
            listing = daemon.request("GET", f"/projects/{project}/photos")
            report["preview_sources"] = Counter(p["analysis"].get("preview_source", "primary") for p in listing)
            thumb = next(p for p in listing if p["filename"] == "embedded-thumbnail.heic")
            assert thumb["analysis"]["preview_source"] == "heic_thumbnail", thumb
            report["incremental"] = daemon.job(project, "analyze")
            assert report["incremental"]["result"]["reused"] == len(entries)
            report["formats"] = Counter(p["format"] for p in listing)
            report["photo_count"] = len(listing)
            report["memory"] = daemon.memory
            report["ok"] = True
    except Exception as error:
        report["error"] = repr(error)
        raise
    finally:
        report["source_hashes_preserved"] = all(digest(Path(p)) == h for p, h in source_hashes.items())
        report["fixture_hashes_preserved"] = all(digest(photos / p) == h for p, h in original.items())
        (args.output / "report.json").write_text(json.dumps(report, indent=2), "utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
