"""Check a Unix beta's files, host startup and synthetic PNG inference."""
import argparse
import binascii
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib


def png():
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", binascii.crc32(kind + data) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 32, 32, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress((b"\0" + bytes([120, 120, 120]) * 32) * 32)) + chunk(b"IEND", b"")


def verify(bundle):
    bundle = bundle.resolve()
    entries = json.loads((bundle / "checksums.json").read_text())
    for entry in entries:
        file = bundle / entry["path"]
        if not file.resolve().is_relative_to(bundle) or file.is_symlink() or hashlib.sha256(file.read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError("Packaged file checksum mismatch")
    binary = bundle / "Iris.app/Contents/MacOS" if (bundle / "Iris.app").exists() else bundle
    with tempfile.TemporaryDirectory(prefix="iris-beta-verify-") as temp:
        root = Path(temp)
        env = {k: v for k, v in os.environ.items() if k not in {"ORT_DYLIB_PATH", "IRIS_MODEL_DIR", "IRIS_DAEMON_PATH", "IRIS_RAW_DECODER_PATH"}}
        env["IRIS_DATA_DIR"] = str(root / "host")
        subprocess.run([binary / "iris-shell", "--verify-host", root / "host.json"], cwd=root, env=env, check=True, timeout=60)
        if not json.loads((root / "host.json").read_text())["ok"]:
            raise ValueError("Native host startup failed")
        photos = root / "photos"
        photos.mkdir()
        (photos / "synthetic.png").write_bytes(png())
        command = [str(binary / "iris-cli"), "--database", str(root / "test.sqlite3"), "--model-dir", str(binary / "models")]
        result = subprocess.run(command + ["scan", str(photos)], cwd=root, env=env, capture_output=True, text=True, check=True, timeout=60)
        project = json.loads(result.stdout)["project"]["id"]
        subprocess.run(command + ["analyze", str(project)], cwd=root, env=env, check=True, timeout=120)
    print("Verified package checksums, native host and synthetic PNG inference")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bundle", type=Path, required=True)
    verify(parser.parse_args().bundle)
