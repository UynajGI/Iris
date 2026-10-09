"""Fetch pinned, licensed model assets; no photographs are transmitted."""
import hashlib
import json
from pathlib import Path
import urllib.request
import io
import zipfile
import ast
import struct

ROOT = Path(__file__).resolve().parents[1] / "models"
REV = "7c463946f31bcb95be863a6dfa4538b235f7248c"
ZOO = "47534e27c9851bb1128ccc0102f1145e27f23f98"

def main():
    ROOT.mkdir(exist_ok=True)
    manifest = json.loads((ROOT / "manifest.json").read_text())
    for item in manifest["models"]:
        target = ROOT / item["file"]
        if not target.exists() or hashlib.sha256(target.read_bytes()).hexdigest() != item["sha256"]:
            data = urllib.request.urlopen(item["url"], timeout=120).read()
            if hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise RuntimeError(f"Hash mismatch: {target.name}")
            target.write_bytes(data)
        print(f"Verified {target.name}: {target.stat().st_size} bytes")
    for name, url in {
        "LICENSE-YuNet.txt": f"https://raw.githubusercontent.com/opencv/opencv_zoo/{ZOO}/models/face_detection_yunet/LICENSE",
        "LICENSE-MediaPipe.txt": f"https://huggingface.co/FreeHugsForRobots/ps-face-landmarks/raw/{REV}/LICENSE.txt",
    }.items():
        (ROOT / name).write_bytes(urllib.request.urlopen(url, timeout=30).read())
    runtime = manifest["runtime"]
    target = ROOT / "onnxruntime.dll"
    if not target.exists() or hashlib.sha256(target.read_bytes()).hexdigest() != runtime["sha256"]:
        data = urllib.request.urlopen(runtime["url"], timeout=120).read()
        if hashlib.sha256(data).hexdigest() != runtime["archive_sha256"]:
            raise RuntimeError("ONNX Runtime archive hash mismatch")
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            prefix = "onnxruntime-win-x64-1.22.0/"
            dll = archive.read(prefix + "lib/onnxruntime.dll")
            if hashlib.sha256(dll).hexdigest() != runtime["sha256"]:
                raise RuntimeError("ONNX Runtime DLL hash mismatch")
            target.write_bytes(dll)
            (ROOT / "LICENSE-ONNXRuntime.txt").write_bytes(archive.read(prefix + "LICENSE"))
            (ROOT / "ThirdPartyNotices-ONNXRuntime.txt").write_bytes(archive.read(prefix + "ThirdPartyNotices.txt"))
    print("Verified ONNX Runtime 1.22.0 Windows x64")
    niqe = manifest["niqe"]
    target = ROOT / "niqe_params.json"
    if not target.exists() or hashlib.sha256(target.read_bytes()).hexdigest() != niqe["sha256"]:
        data = urllib.request.urlopen(niqe["url"], timeout=30).read()
        if hashlib.sha256(data).hexdigest() != niqe["source_sha256"]:
            raise RuntimeError("NIQE pristine statistics hash mismatch")
        values = {}
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for name in ["mu_pris_param", "cov_pris_param", "gaussian_window"]:
                raw = archive.read(name + ".npy")
                if raw[:8] != b"\x93NUMPY\x01\x00":
                    raise RuntimeError("Unexpected NPY statistics encoding")
                size = struct.unpack("<H", raw[8:10])[0]
                header = ast.literal_eval(raw[10:10 + size].decode())
                if header["descr"] != "<f8":
                    raise RuntimeError("Unexpected NIQE statistics dtype or layout")
                payload = raw[10 + size:]
                numbers = list(struct.unpack("<" + "d" * (len(payload) // 8), payload))
                if header["fortran_order"]:
                    rows, cols = header["shape"]
                    numbers = [numbers[col * rows + row] for row in range(rows) for col in range(cols)]
                values[name] = numbers
        converted = json.dumps(values, separators=(",", ":")).encode()
        if hashlib.sha256(converted).hexdigest() != niqe["sha256"]:
            raise RuntimeError("Converted NIQE statistics hash mismatch")
        target.write_bytes(converted)
    (ROOT / "LICENSE-BasicSR.txt").write_bytes(urllib.request.urlopen(niqe["license_url"], timeout=30).read())
    print("Verified classical NIQE pristine statistics")

if __name__ == "__main__":
    main()
