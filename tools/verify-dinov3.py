"""Verify pinned DINOv3 bytes and CPU tensor execution without decoding photos.

Real JPEG/exposure smoke uses the Rust decoder/preprocessor via
vision::dinov3::tests::real_dinov3_cpu_embedding (explicit ignored test).
No network calls, labels, threshold calibration, or semantic quality claims.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time
import numpy as np
import onnxruntime as ort

SHA256 = "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c"


def verify(path):
    model = path.read_bytes()
    if hashlib.sha256(model).hexdigest() != SHA256:
        raise ValueError("Pinned DINOv3 SHA-256 mismatch")
    options = ort.SessionOptions()
    options.intra_op_num_threads = options.inter_op_num_threads = 1
    session = ort.InferenceSession(model, sess_options=options, providers=["CPUExecutionProvider"])
    if [value.name for value in session.get_inputs()] != ["pixel_values"] or [value.name for value in session.get_outputs()] != ["last_hidden_state", "pooler_output"]:
        raise ValueError("Unexpected DINOv3 input/output contract")
    tensor = np.zeros((1, 3, 224, 224), dtype=np.float32)
    started = time.perf_counter()
    outputs = [session.run(["pooler_output"], {"pixel_values": tensor})[0] for _ in range(2)]
    if any(value.shape != (1, 384) or not np.isfinite(value).all() or np.linalg.norm(value) <= 1e-12 for value in outputs):
        raise ValueError("Invalid DINOv3 embedding")
    if not np.array_equal(*outputs):
        raise ValueError("Repeated DINOv3 CPU inference differed")
    unit = outputs[0] / np.linalg.norm(outputs[0])
    return {"ok": True, "sha256": SHA256, "bytes": len(model), "runtime": ort.__version__, "providers": session.get_providers(), "input_shape": list(tensor.shape), "output_shape": list(unit.shape), "unit_norm": float(np.linalg.norm(unit)), "repeat_identical": True, "two_inferences_seconds": time.perf_counter() - started, "photos_uploaded": False, "quality_validated": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    root = Path(__file__).resolve().parents[1]
    parser.add_argument("--model", type=Path, default=root / "models/optional/dinov3_vits16.onnx")
    parser.add_argument("--output", type=Path, default=root / "artifacts/dinov3-onnx-smoke.json")
    args = parser.parse_args()
    report = verify(args.model)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
