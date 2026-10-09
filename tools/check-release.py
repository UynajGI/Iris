"""Fail a release when its tag and application versions disagree."""
import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def check(tag, root=ROOT):
    if not re.fullmatch(r"v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?", tag):
        raise ValueError("Expected a version tag such as v0.1.0-beta")
    expected = tag[1:]
    for name in ("Cargo.toml", "apps/shell/src-tauri/Cargo.toml", "components/raw-decoder/Cargo.toml"):
        version = re.search(r'^version = "([^"]+)"', (root / name).read_text(), re.M)[1]
        if version != expected:
            raise ValueError(f"{name}: version differs from {tag}")
    for name in ("apps/shell/package.json", "apps/shell/package-lock.json", "apps/shell/src-tauri/tauri.conf.json"):
        if json.loads((root / name).read_text())["version"] != expected:
            raise ValueError(f"{name}: version differs from {tag}")
    return expected


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    print(check(parser.parse_args().tag))
