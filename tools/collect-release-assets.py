"""Require every supported release artifact before checksums or draft creation."""
# SPDX-License-Identifier: GPL-3.0-or-later
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def expected_assets(version):
    base = f"Iris-{version}"
    return {f"{base}-source.zip", f"{base}-windows-x64.zip", f"{base}-windows-x64-setup.exe",
            f"{base}-linux-x64.tar.gz", f"{base}-linux-x64.deb",
            f"{base}-macos-arm64.tar.gz", f"{base}-macos-arm64.pkg",
            f"{base}-macos-x64.tar.gz", f"{base}-macos-x64.pkg"}


def collect(directory, version):
    names = expected_assets(version)
    found = {p.name for p in directory.iterdir() if p.name != "SHA256SUMS.txt"}
    if found != names:
        raise ValueError(f"Incomplete or unexpected release set; missing={sorted(names - found)}, unexpected={sorted(found - names)}")
    lines = []
    for name in sorted(names):
        path = directory / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Invalid release asset: {name}")
        with path.open("rb") as stream:
            sha = hashlib.file_digest(stream, "sha256").hexdigest()
        lines.append(f"{sha}  {name}\n")
    checksum = directory / "SHA256SUMS.txt"
    if checksum.exists() or checksum.is_symlink():
        raise ValueError("Checksum list already exists; choose a fresh collection directory")
    checksum.write_text("".join(lines), encoding="utf-8")
    return names


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    version = json.loads((ROOT / "apps/shell/package.json").read_text(encoding="utf-8"))["version"]
    print(f"Verified {len(collect(args.directory, version))} assets for {version}")
