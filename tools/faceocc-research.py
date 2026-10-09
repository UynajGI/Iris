"""Isolated FaceOcc CPU/export research; never used by the product model loader.

py -3.13 tools/faceocc-research.py acquire
py -3.13 tools/faceocc-research.py verify
Requires existing torch/torchvision/safetensors/onnxruntime and isolated SMP/timm/onnx.
No pickle loading, remote-code execution, photographs, or pretrained backbone fetch.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import json
import math
import platform
import statistics
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "artifacts" / "faceocc-research"
REPO = "mertakin/FaceOcc"
REVISION = "03f229dc75fa14ae480cca9810f983912c2730ad"
WEIGHT_SHA256 = "5d6d27a9cb221692425840a80eb4efc07e6245670b233e2eb618e83fd9b7caf6"
WEIGHT_BYTES = 57_377_516
FILES = ["README.md", "LICENSE", "NOTICE.md", "config.json", "training_config.json", "model.safetensors"]


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def acquire() -> None:
    if subprocess.run(["git", "check-ignore", "--quiet", str(OUT / "probe")], cwd=ROOT).returncode:
        raise RuntimeError("Research directory must be Git-ignored before acquiring weights")
    OUT.mkdir(parents=True, exist_ok=True)
    records = []
    for filename in FILES:
        url = f"https://huggingface.co/{REPO}/resolve/{REVISION}/{filename}"
        path = OUT / filename
        if filename == "model.safetensors" and path.exists() and digest(path) == WEIGHT_SHA256:
            print("Reusing verified safetensors", flush=True)
        else:
            limit = WEIGHT_BYTES if filename == "model.safetensors" else 128 * 1024
            for attempt in range(3):
                temporary = path.with_suffix(path.suffix + ".partial")
                try:
                    request = urllib.request.Request(url, headers={"User-Agent": "IrisVision-local-research/0.1"})
                    with urllib.request.urlopen(request, timeout=45) as response, temporary.open("wb") as stream:
                        count = 0
                        while chunk := response.read(1024 * 1024):
                            count += len(chunk)
                            if count > limit:
                                raise ValueError(f"Unexpectedly large artifact: {filename}")
                            stream.write(chunk)
                    if filename == "model.safetensors" and (temporary.stat().st_size != WEIGHT_BYTES or digest(temporary) != WEIGHT_SHA256):
                        raise ValueError("FaceOcc safetensors size/SHA-256 differs from pinned publisher metadata")
                    temporary.replace(path)
                    break
                except Exception:
                    if temporary.exists():
                        temporary.unlink()
                    if attempt == 2:
                        raise
                    time.sleep(attempt + 1)
        records.append({"file": filename, "url": url, "bytes": path.stat().st_size, "sha256": digest(path)})
        print(f"Acquired {filename}: {path.stat().st_size} bytes", flush=True)
    card = (OUT / "README.md").read_text(encoding="utf-8")
    if "license: mit" not in card or "Distributed under the MIT License" not in card:
        raise ValueError("Pinned model card no longer supplies the expected model license declaration")
    save_json(OUT / "provenance.json", {
        "repository": REPO, "revision": REVISION, "files": records,
        "scope": "Local synthetic-input CPU/export technical validation only; no product integration, redistribution, training data or private photos.",
        "license_evidence": "Pinned model card declares MIT; LICENSE and NOTICE retained. Third-party resource terms are not superseded. This is not distribution clearance.",
        "upstream_weight_grant": "https://github.com/face3d0725/FaceExtraction/blob/e75d4a83a696bd7379128319244ef6e5e7885fc8/README.md#L43",
    })


def benchmark(call, warmups: int = 3, repeats: int = 15) -> dict:
    for _ in range(warmups):
        call()
    durations = []
    for _ in range(repeats):
        start = time.perf_counter()
        call()
        durations.append((time.perf_counter() - start) * 1000)
    ordered = sorted(durations)
    return {"warmups": warmups, "repeats": repeats, "median_ms": statistics.median(durations), "p95_nearest_rank_ms": ordered[math.ceil(0.95 * len(ordered)) - 1], "samples_ms": durations}


def verify() -> None:
    sys.path.insert(0, str(OUT / "python-deps"))
    import numpy as np
    import onnx
    import onnxruntime as ort
    import segmentation_models_pytorch as smp
    import torch
    from safetensors.torch import load_file

    provenance = json.loads((OUT / "provenance.json").read_text(encoding="utf-8"))
    if provenance["revision"] != REVISION:
        raise ValueError("Unexpected model revision")
    for item in provenance["files"]:
        path = OUT / item["file"]
        if digest(path) != item["sha256"]:
            raise ValueError(f"Research artifact changed: {item['file']}")
    if digest(OUT / "model.safetensors") != WEIGHT_SHA256:
        raise ValueError("Pinned weight SHA-256 mismatch")
    config = json.loads((OUT / "config.json").read_text(encoding="utf-8"))
    expected = {"architecture": "Unet", "encoder": "resnet18", "in_channels": 3, "classes": 1, "input_size": [256, 256], "output": "logits", "probability_threshold": 0.5}
    if any(config.get(key) != value for key, value in expected.items()):
        raise ValueError("Unsupported FaceOcc inference configuration")

    torch.set_num_threads(1)
    torch.set_num_interop_threads(1)
    torch.manual_seed(20261006)
    network = smp.Unet(encoder_name="resnet18", encoder_weights=None, encoder_depth=5,
        decoder_use_norm="batchnorm", decoder_channels=(256, 128, 64, 32, 16),
        decoder_interpolation="nearest", in_channels=3, classes=1, activation=None)
    # No torch.load or trust_remote_code: only safe tensor values into local library code.
    state = load_file(str(OUT / "model.safetensors"), device="cpu")
    network.load_state_dict(state, strict=True)
    network.eval()

    class NormalizedModel(torch.nn.Module):
        def __init__(self):
            super().__init__()
            self.network = network
            self.register_buffer("mean", torch.tensor(config["normalization"]["mean"]).reshape(1, 3, 1, 1))
            self.register_buffer("std", torch.tensor(config["normalization"]["std"]).reshape(1, 3, 1, 1))

        def forward(self, image):
            return self.network((image - self.mean) / self.std)

    model = NormalizedModel().eval()
    shape = (1, 3, 256, 256)
    rng = np.random.default_rng(20261006)
    gradient = np.broadcast_to(np.linspace(0, 1, 256, dtype=np.float32), shape).copy()
    inputs = {"black": np.zeros(shape, np.float32), "white": np.ones(shape, np.float32),
        "gray": np.full(shape, 0.5, np.float32), "gradient": gradient,
        "seeded_noise": rng.random(shape, dtype=np.float32)}
    reference = {}
    with torch.inference_mode():
        for name, pixels in inputs.items():
            values = model(torch.from_numpy(pixels)).numpy()
            if values.shape != (1, 1, 256, 256) or not np.isfinite(values).all():
                raise ValueError("PyTorch output violated the finite logits contract")
            reference[name] = values
        export_path = OUT / "faceocc-research.onnx"
        torch.onnx.export(model, (torch.from_numpy(inputs["gradient"]),), str(export_path),
            input_names=["rgb_0_1"], output_names=["visible_face_logits"],
            opset_version=17, dynamo=False, external_data=False)
    graph = onnx.load(str(export_path), load_external_data=False)
    onnx.checker.check_model(graph, full_check=True)
    if any(initializer.data_location == onnx.TensorProto.EXTERNAL for initializer in graph.graph.initializer):
        raise ValueError("Research ONNX unexpectedly depends on external data")
    options = ort.SessionOptions()
    options.intra_op_num_threads = 1
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(str(export_path), sess_options=options, providers=["CPUExecutionProvider"])
    comparisons = []
    for name, pixels in inputs.items():
        output = session.run(None, {"rgb_0_1": pixels})[0]
        target = reference[name]
        if output.shape != target.shape:
            raise ValueError("ONNX output shape differs from PyTorch")
        diff = np.abs(output - target)
        passed = bool(np.allclose(output, target, atol=1e-4, rtol=1e-4))
        comparisons.append({"input": name, "shape": list(output.shape), "finite": bool(np.isfinite(output).all()),
            "max_abs_logit_error": float(diff.max()), "mean_abs_logit_error": float(diff.mean()),
            "mask_disagreement_pixels_at_probability_0_5": int(np.count_nonzero((output >= 0) != (target >= 0))),
            "allclose_atol_1e_4_rtol_1e_4": passed})
    tensor = torch.from_numpy(inputs["seeded_noise"])
    with torch.inference_mode():
        pytorch_timing = benchmark(lambda: model(tensor))
    onnx_timing = benchmark(lambda: session.run(None, {"rgb_0_1": inputs["seeded_noise"]}))
    packages = {name: importlib.metadata.version(name) for name in ["torch", "torchvision", "segmentation-models-pytorch", "timm", "safetensors", "onnx", "onnxruntime", "numpy"]}
    report = {"scope": "Synthetic CPU/export equivalence only. No occlusion accuracy or eye visibility validation.",
        "repository": REPO, "revision": REVISION, "weight_sha256": WEIGHT_SHA256,
        "onnx_sha256": digest(export_path), "onnx_bytes": export_path.stat().st_size,
        "python": sys.version, "platform": platform.platform(), "processor": platform.processor(), "packages": packages,
        "input_contract": "float32 RGB NCHW [1,3,256,256], 0..1; normalization included in exported graph",
        "output_contract": "float32 [1,1,256,256] visible-face logits; zero logit is probability 0.5",
        "threads_per_runtime": 1, "onnx_opset": 17, "onnx_ir_version": graph.ir_version,
        "providers": session.get_providers(), "tensor_count": len(state), "comparisons": comparisons,
        "pytorch_cpu": pytorch_timing, "onnx_cpu": onnx_timing,
        "passed": all(row["finite"] and row["allclose_atol_1e_4_rtol_1e_4"] for row in comparisons)}
    save_json(OUT / "verification.json", report)
    print(json.dumps({key: report[key] for key in ["passed", "weight_sha256", "onnx_sha256", "comparisons", "pytorch_cpu", "onnx_cpu"]}, indent=2), flush=True)
    if not report["passed"]:
        raise RuntimeError("ONNX/PyTorch agreement tolerance failed; see verification.json")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["acquire", "verify"])
    args = parser.parse_args()
    if args.command == "acquire":
        acquire()
    else:
        verify()
