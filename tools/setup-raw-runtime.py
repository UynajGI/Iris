"""Provision pinned standalone ExifTool for local RAW previews; no photo upload."""
import argparse
import base64
import hashlib
import io
import json
from pathlib import Path
import tarfile
import urllib.request

VERSION = "13.59.3"
URL = f"https://registry.npmjs.org/exiftool-vendored.exe/-/exiftool-vendored.exe-{VERSION}.tgz"
INTEGRITY = "F5hpk1yVGZSDHjSKsyumyniY56IM3zawKOejOqtyT2r476fvWrUxpBmWT7Ena104TGPt/2+448Q/vrVt+yOzwQ=="


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "models/raw")
    args = parser.parse_args()
    data = urllib.request.urlopen(URL, timeout=120).read(48 * 1024 * 1024)
    if base64.b64encode(hashlib.sha512(data).digest()).decode() != INTEGRITY:
        raise ValueError("ExifTool archive integrity mismatch")
    args.output.mkdir(parents=True, exist_ok=True)
    payload = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for entry in archive.getmembers():
            parts = Path(entry.name).parts
            if not entry.isfile():
                continue
            if not parts or parts[0] != "package" or ".." in parts:
                raise ValueError("unexpected archive path")
            relative = Path(*parts[1:]).as_posix()
            if relative in payload or ":" in relative:
                raise ValueError("duplicate or invalid runtime entry")
            target = args.output.joinpath(*parts[1:])
            content = archive.extractfile(entry).read()
            if target.exists() and target.read_bytes() != content:
                raise ValueError(f"refusing to overwrite changed file: {target}")
            payload[relative] = content
    for path in args.output.rglob("*"):
        if path.is_symlink() or path.is_junction():
            raise ValueError("linked entries cannot enter the RAW runtime")
        if path.is_file() and path.relative_to(args.output).as_posix() not in payload and path != args.output / "manifest.json":
            raise ValueError("unlisted file in RAW runtime output")
    for relative, content in payload.items():
        target = args.output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(content)
    if not (args.output / "bin/exiftool.exe").is_file():
        raise ValueError("standalone ExifTool missing")
    files = [{"path": name, "size": len(content), "sha256": hashlib.sha256(content).hexdigest()}
             for name, content in sorted(payload.items())]
    (args.output / "manifest.json").write_text(json.dumps({"source": URL, "version": VERSION,
        "archive_sha512_base64": INTEGRITY, "files": files}, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"runtime": str(args.output), "verified_archive": True, "files": len(files)}))


if __name__ == "__main__":
    main()
