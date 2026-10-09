"""Specialize only tensor shapes of the approved DINO graph for DirectML, offline."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path

SPEC = importlib.util.spec_from_file_location("setup_dinov3", Path(__file__).with_name("setup-dinov3.py"))
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)
SHA256 = "cb115eebbe83bb4f592845203dbee5f98253a63292210ff9ab3e73062c46dde5"
SIZE = 86415189
FILE = "dinov3_vits16_directml.onnx"


def convert(source):
    import numpy as np
    import onnx
    import onnxruntime as ort
    if onnx.__version__ != "1.23.2":
        raise RuntimeError("Use onnx==1.23.2 for deterministic serialization")
    original_bytes = BASE.checked(source.read_bytes(), BASE.SIZE, BASE.SHA256, "original model")
    graph = onnx.load_model_from_string(original_bytes)
    traced = copy.deepcopy(graph)
    types = {v.name: v.type.tensor_type.elem_type for v in list(graph.graph.value_info) + list(graph.graph.output)}
    reshapes = [n for n in graph.graph.node if n.op_type == "Reshape"]
    for node in reshapes:
        traced.graph.output.append(onnx.helper.make_tensor_value_info(node.output[0], types[node.output[0]], None))
    options = ort.SessionOptions()
    options.log_severity_level = 3
    session = ort.InferenceSession(traced.SerializeToString(), options, providers=["CPUExecutionProvider"])
    data = np.zeros((1, 3, 224, 224), np.float32)
    outputs = [n.output[0] for n in reshapes]
    shapes = [v.shape for v in session.run(outputs, {"pixel_values": data})]
    random = np.random.default_rng(20261007).normal(size=data.shape).astype(np.float32)
    assert shapes == [v.shape for v in session.run(outputs, {"pixel_values": random})]
    for dim, size in zip(graph.graph.input[0].type.tensor_type.shape.dim, data.shape):
        dim.ClearField("dim_param")
        dim.dim_value = size
    del graph.graph.value_info[:]
    for node, shape in zip(reshapes, shapes):
        name = node.name + "_iris_fixed_shape"
        graph.graph.initializer.append(onnx.numpy_helper.from_array(np.asarray(shape, dtype=np.int64), name))
        node.input[1] = name
    # Every original initializer (including all learned weights) is byte-identical.
    original = onnx.load_model_from_string(original_bytes)
    assert all(a.SerializeToString() == b.SerializeToString()
               for a, b in zip(original.graph.initializer, graph.graph.initializer))
    onnx.checker.check_model(graph)
    serialized = graph.SerializeToString(deterministic=True)
    baseline = ort.InferenceSession(original_bytes, options, providers=["CPUExecutionProvider"])
    specialized = ort.InferenceSession(serialized, options, providers=["CPUExecutionProvider"])
    errors = []
    for sample in [data, random]:
        a = baseline.run(["pooler_output"], {"pixel_values": sample})[0]
        b = specialized.run(["pooler_output"], {"pixel_values": sample})[0]
        errors.append(float(np.max(np.abs(a - b))))
    assert max(errors) < 1e-4, errors
    return serialized, {"original_sha256": BASE.SHA256, "conversion": "fixed 1x3x224x224 input; explicit Reshape sizes; no weight edits or quantization",
        "reshapes": len(reshapes), "original_initializers_preserved": len(original.graph.initializer),
        "onnx_version": onnx.__version__, "shape_trace_ort_version": ort.__version__, "synthetic_cpu_max_absolute_errors": errors}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parents[1] / "models/optional/dinov3_vits16.onnx")
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "models/optional" / FILE)
    args = parser.parse_args()
    data, metadata = convert(args.source)
    digest = hashlib.sha256(data).hexdigest()
    BASE.checked(data, SIZE, SHA256, "DirectML graph")
    if args.output.exists() and args.output.read_bytes() != data:
        raise ValueError("Refusing to overwrite a different existing artifact")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(data)
    metadata.update(file=FILE, sha256=digest, bytes=len(data), license="Meta DINOv3 License; preserve LICENSE-DINOv3.md", accuracy_validated=False)
    args.output.with_suffix(".metadata.json").write_text(json.dumps(metadata, indent=2) + "\n", "utf-8")
    print(json.dumps(metadata))


if __name__ == "__main__":
    main()
