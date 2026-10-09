"""Compare Rust NIQE with BasicSR's published MATLAB-compatible baboon value.

Development-only dependencies: numpy, Pillow. Downloads one public benchmark,
never accesses or uploads user photographs. Generated fixtures stay in artifacts/.
"""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import urllib.request

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
REV = "8d56e3a045f9fb3e1d8872f92ee4a4f07f886b0a"
URL = f"https://raw.githubusercontent.com/XPixelGroup/BasicSR/{REV}/test_scripts/data/baboon.png"

def main():
    data = urllib.request.urlopen(URL, timeout=30).read()
    rgb = np.asarray(Image.open(io.BytesIO(data)).convert("RGB"), dtype=np.float32)
    # Reproduce BasicSR to_y_channel: input is BGR float32 in [0,255],
    # converted to [0,1], then MATLAB YCbCr weights, then back to [0,255].
    y = np.dot(rgb / 255., np.array([65.481, 128.553, 24.966])) + 16.
    y = np.round(y.astype(np.float32)).astype(np.uint8)
    directory = ROOT / "artifacts" / "niqe-validation"
    directory.mkdir(parents=True, exist_ok=True)
    fixture = directory / "baboon-luma.bin"
    fixture.write_bytes(struct.pack("<II", y.shape[1], y.shape[0]) + y.tobytes())
    env = dict(os.environ, IRIS_NIQE_LUMA_FILE=str(fixture))
    result = subprocess.run(
        ["cargo", "test", "-p", "iris-core", "vision::niqe::tests::external_reference", "--", "--ignored", "--nocapture"],
        cwd=ROOT, env=env, capture_output=True, text=True, check=True,
    )
    match = re.search(r"NIQE_REFERENCE_SCORE=([\d.]+)", result.stdout)
    if not match:
        raise RuntimeError(result.stdout + result.stderr)
    actual = float(match.group(1))
    expected = 5.7295763  # BasicSR niqe.py published reference; MATLAB 5.72957338.
    report = {"reference_source": URL, "image_sha256": hashlib.sha256(data).hexdigest(),
              "expected_basicsr": expected, "expected_matlab": 5.72957338,
              "actual_rust": actual, "absolute_error": abs(actual-expected), "tolerance": 0.002}
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))
    assert report["absolute_error"] < report["tolerance"], "Rust NIQE differs from published reference"

if __name__ == "__main__":
    main()
