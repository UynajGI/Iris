"""Probe already-installed pinned DINO graphs; no model downloads or conversions.

Synthetic tensors isolate inference overhead. They do not measure photo decoding,
portrait accuracy, or prove that every operator runs on the selected GPU.
"""

import argparse
import hashlib
import json
import statistics
import time
from pathlib import Path

import numpy as np
import onnxruntime as ort

parser = argparse.ArgumentParser()
parser.add_argument("--models", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
graphs = [
    ("dinov3_vits16.onnx", "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c"),
    (
        "dinov3_vits16_directml.onnx",
        "cb115eebbe83bb4f592845203dbee5f98253a63292210ff9ab3e73062c46dde5",
    ),
]
report = {
    "runtime": ort.__version__,
    "available_providers": ort.get_available_providers(),
    "synthetic_inputs": True,
    "sessions": [],
}
for graph, expected in graphs:
    path = args.models / "optional" / graph
    with path.open("rb") as source:
        actual = hashlib.file_digest(source, "sha256").hexdigest()
    assert actual == expected, "Unapproved model bytes"
    providers = (
        [("CPUExecutionProvider", {})]
        if "directml" not in graph
        else [("DmlExecutionProvider", {"device_id": str(device)}) for device in [0, 1]]
    )
    for provider, options in providers:
        if provider not in ort.get_available_providers():
            continue
        session_options = ort.SessionOptions()
        session_options.intra_op_num_threads = 1
        session_options.inter_op_num_threads = 1
        session_options.enable_mem_pattern = False
        session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        session = ort.InferenceSession(
            str(path),
            sess_options=session_options,
            providers=[(provider, options), "CPUExecutionProvider"],
        )
        entry = {
            "graph": graph,
            "sha256": actual,
            "requested_provider": provider,
            "options": options,
            "registered_providers": session.get_providers(),
            "input_shape": session.get_inputs()[0].shape,
            "batches": [],
        }
        for batch in [1, 2, 4, 8]:
            values = np.zeros([batch, 3, 224, 224], dtype=np.float32)
            item = {"size": batch}
            try:
                for _ in range(3):
                    result = session.run(["pooler_output"], {"pixel_values": values})[0]
                assert list(result.shape) == [batch, 384] and np.isfinite(result).all()
                samples = []
                for _ in range(20):
                    start = time.perf_counter()
                    session.run(["pooler_output"], {"pixel_values": values})
                    samples.append((time.perf_counter() - start) * 1000)
                item.update(
                    supported=True,
                    median_ms=statistics.median(samples),
                    photos_per_second=batch * 1000 / statistics.median(samples),
                )
            except Exception as error:
                item.update(supported=False, error=str(error)[:800])
            entry["batches"].append(item)
        report["sessions"].append(entry)
args.output.parent.mkdir(parents=True, exist_ok=True)
args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({"sessions": len(report["sessions"]), "output": str(args.output)}))
