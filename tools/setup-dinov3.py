"""Install a pinned public DINOv3 ONNX conversion for explicit local use.

Meta's DINOv3 license applies (not Apache/MIT); it permits redistribution with
the agreement. This uses the ungated onnx-community conversion, never requests
the gated Meta checkpoint, accepts a gate, uses a token, or uploads photographs.
Requires onnx==1.23.2. Default models remain untouched.
"""
import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request

REPOSITORY = "onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX"
REVISION = "48988dfe73065df8d6f5ccc0edc7c8bcf307de41"
SHA256 = "f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c"
SIZE = 86474453
SOURCES = {
    "onnx/model.onnx": (137969, "bb75e9e30ff382ecdbd150266445ab41272be4acc85fd3563218dd89781e36da"),
    "onnx/model.onnx_data": (86347776, "1eff0bb9f4fdef831ca61c8bc2b5c88d8cc21ef4756d3b483d9b9c15d8d5d27f"),
    "LICENSE.md": (7502, "aa878c2fe56729d87f735e1cab375b27079aa2ef5f9a06e85456a4ba2c89e7b8"),
    "config.json": (835, "e9f41d1c030ec6589280ba417e630b916d6e4d0e38a3efae15c11aa12c22c984"),
    "preprocessor_config.json": (585, "960c41d1f3a7778b936365769a2d90550b318a6c0a53a0296957adacfe5e0dd7"),
}


def checked(data, size, digest, label):
    if len(data) != size or hashlib.sha256(data).hexdigest() != digest:
        raise ValueError(f"DINOv3 size/SHA-256 mismatch: {label}")
    return data


def setup(output, source_dir=None):
    import onnx
    if onnx.__version__ != "1.23.2":
        raise RuntimeError("Use onnx==1.23.2 for the pinned deterministic single-file serialization")
    output.mkdir(parents=True, exist_ok=True)
    metadata = {
        "model_id": "dinov3_vits16_lvd1689m",
        "file": "dinov3_vits16.onnx", "sha256": SHA256, "bytes": SIZE,
        "source_repository": REPOSITORY, "source_revision": REVISION,
        "base_model": "facebook/dinov3-vits16-pretrain-lvd1689m",
        "license": "Meta DINOv3 License (August 19, 2025)",
        "license_url": "https://github.com/facebookresearch/dinov3/blob/main/LICENSE.md",
        "license_note": "Public community conversion under the upstream DINOv3 license; not an official Meta ONNX export. Retain LICENSE-DINOv3.md on redistribution.",
        "conversion": "onnx 1.23.2: embed external tensors; deterministic protobuf serialization; no quantization or graph rewrite",
        "preprocessing_id": "dinov3_vits16_rgb224_triangle_imagenet_pooler_l2_v1",
        "input": "RGB preview <=1280; full-image 224x224 triangle resize, no crop; NCHW float32 /255; ImageNet mean/std",
        "output": "pooler_output [1,384] CLS token, L2 normalized by Iris",
        "accuracy_validated": False, "sources": {},
    }
    with tempfile.TemporaryDirectory(prefix="dinov3-") as temporary:
        stage = Path(temporary)
        for name, (size, digest) in SOURCES.items():
            url = f"https://huggingface.co/{REPOSITORY}/resolve/{REVISION}/{name}"
            if source_dir is not None:
                data = (source_dir / Path(name).name).read_bytes()
            else:
                with urllib.request.urlopen(url, timeout=180) as response:
                    data = response.read(size + 1)
            checked(data, size, digest, name)
            (stage / Path(name).name).write_bytes(data)
            metadata["sources"][name] = {"url": url, "sha256": digest, "bytes": size}
        graph = onnx.load(stage / "model.onnx")
        onnx.external_data_helper.convert_model_from_external_data(graph)
        data = checked(graph.SerializeToString(deterministic=True), SIZE, SHA256, "merged model")
        # Verify the whole merged graph before exposing it as an installed artifact.
        onnx.checker.check_model(graph)
        (output / "LICENSE-DINOv3.md").write_bytes((stage / "LICENSE.md").read_bytes())
        (output / "dinov3_vits16.onnx").write_bytes(data)
        (output / "dinov3_vits16.metadata.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parents[1] / "models/optional")
    parser.add_argument("--source-dir", type=Path, help="Optional offline directory of previously downloaded, pinned source files")
    args = parser.parse_args()
    metadata = setup(args.output_dir, args.source_dir)
    print(json.dumps({"ok": True, "path": str(args.output_dir / metadata["file"]), "sha256": SHA256, "bytes": SIZE}))


if __name__ == "__main__":
    main()
