# Optional local DINOv3 embeddings

This integration uses **DINOv3 ViT-S/16 pretrained on LVD-1689M**, not DINOv2 or
CLIP. The default analysis path does not use or download it. Enabling semantic
grouping requires the pinned local artifact and an explicit similarity threshold;
no threshold is claimed to be calibrated on the wedding photographs.

## Source and license

- Official architecture/documentation: https://github.com/facebookresearch/dinov3
- Official model: https://huggingface.co/facebook/dinov3-vits16-pretrain-lvd1689m
- Public community ONNX conversion:
  https://huggingface.co/onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX/tree/48988dfe73065df8d6f5ccc0edc7c8bcf307de41
- Meta DINOv3 License, August 19, 2025:
  https://github.com/facebookresearch/dinov3/blob/main/LICENSE.md

The official checkpoint repository has a manual access gate. The setup tool does
not access it, submit personal information, accept its gate, or use credentials.
It downloads the separately public, ungated onnx-community conversion, which
identifies the official DINOv3 model as its base and includes the upstream license.
This is a community conversion, not an official Meta ONNX export. The custom
DINOv3 license is not Apache/MIT; section 1 permits use and redistribution under
its conditions, including supplying the agreement. Preserve `LICENSE-DINOv3.md`
when redistributing these optional weights. Publication of research results must
acknowledge DINO Materials. All other license conditions continue to apply.

## Install and verify

The setup dependency is `onnx==1.23.2`; it performs no model conversion training or
quantization. It embeds the external tensors into the protobuf with deterministic
serialization and verifies the resulting fixed hash before installing it.

```powershell
python tools/setup-dinov3.py
python tools/verify-dinov3.py
cargo test -p iris-core vision::dinov3::tests
$env:IRIS_TEST_PHOTOS = 'C:\Photos'
$env:IRIS_DINOV3_REPORT = (Join-Path (Get-Location) 'artifacts/dinov3-rust-smoke.json')
cargo test -p iris-core real_dinov3_cpu_embedding -- --ignored --nocapture
```

`--source-dir` accepts previously downloaded source files for an offline setup;
all five source sizes and hashes are still checked. The resulting optional model
is `models/optional/dinov3_vits16.onnx`, **86,474,453 bytes**, SHA-256
`f0729237db38a442aa377f45ba38a223d9e11ea27f5bc9b6d5a71588d9ae3c2c`.
The source graph and external tensor bytes are 137,969 and 86,347,776 bytes;
their exact hashes and URLs are pinned in `setup-dinov3.py` and copied into
`dinov3_vits16.metadata.json`. The default model manifest is unchanged.

## Separate offline distribution

`python tools/package-dinov3.py --output dist/optional/<new-directory>` creates a
standalone folder and ZIP from the installed pinned artifact. It verifies model,
license and provenance before copying, includes only those three payload files,
and adds deployment instructions, a PowerShell verifier and SHA-256 manifest.
The destination must be new. It never installs or enables the model and never
copies SCRFD or other optional directory contents. The general portable package
includes the default models and verified HEIC media closure, while all optional
models remain excluded.

## Tensor contract

The model has 12 transformer layers, 6 attention heads, 4 register tokens, RoPE,
and 384 feature channels, matching official ViT-S/16. The exported graph is ONNX
IR 10 / opset 20. Input `pixel_values` is NCHW float32. Outputs are
`last_hidden_state` (patch, register and CLS tokens) and `pooler_output` (CLS).
Only `pooler_output [1,384]` is used, followed by L2 normalization in Rust.

The Rust module receives an existing RGB analysis preview with longest edge
at most 1280; it never opens a source image or performs full-resolution decoding.
It resizes the entire image to 224×224 with the `image` crate's Triangle filter,
without cropping, then divides channels by 255 and normalizes with ImageNet
mean `(0.485,0.456,0.406)` and standard deviation `(0.229,0.224,0.225)`.
224×224 and bilinear resizing follow this conversion's preprocessing metadata.
The resize implementation is a versioned local choice, not a claim of bitwise
torchvision/Pillow parity. Contract ID:
`dinov3_vits16_rgb224_triangle_imagenet_pooler_l2_v1`.

The single-file bytes are checked before ORT loads them from memory, so no external
tensor path can escape verification. Loading uses one intra/inter-op thread,
validates graph input/output shapes, and performs a startup inference. The low-level
decoder rejects missing, corrupt, unapproved, nonfinite or zero-norm results.
The daemon computes embeddings only for time/pHash/aspect candidates and preserves
quality features and matching vectors when a semantic threshold changes.
At job level, missing/unavailable embeddings explicitly return `phash_fallback`
with a reason; corrupt/unapproved model bytes remain fatal before cache reuse.
DirectML is optional. The original dynamic graph has incompatible Reshape behavior
on the local adapters. `python tools/setup-dinov3-directml.py` creates an additional
approved fixed-shape graph offline: input 1x3x224x224, 81 explicit Reshape sizes,
all 213 original initializers preserved byte for byte. It uses ONNX 1.23.2 and
CPU ORT for shape tracing (validated with ORT 1.30.0); the output must match the
pinned 86,415,189-byte SHA-256
`cb115eebbe83bb4f592845203dbee5f98253a63292210ff9ab3e73062c46dde5`.
CPU continues using the original graph. Both installed artifacts are validated
before cache reuse; a corrupt GPU graph is fatal. A missing GPU graph or failed
GPU startup retries CPU with an explicit warning. No network inference occurs.

Use `package-dinov3.py --include-directml --output <new-directory>` to include the
additional graph and provenance in the separate optional payload (five model/
license/provenance files, seven checksummed payload/instruction files total).
This remains governed by the Meta license. Both local GPU indices completed real
DINO inference on 100 authorized photographs plus one duplicate, with 44 candidate
vectors and 20,565 DINO GPU profile events each; minimum cosine to CPU exceeded
0.99999995. This is successful GPU completion with possible CPU operators, not an
all-GPU or acceleration claim. Detailed evidence: [validation overview](../docs/validation.md).

## Evidence limits

`verify-dinov3.py` tests repeated CPU tensor execution and unit normalization.
The explicit Rust smoke uses the application's actual ORT and JPEG preview
decoder; when `IRIS_TEST_PHOTOS` is provided, it reads two JPGs, checks repeat
consistency, compares an in-memory exposure variant and a different photograph,
and checks source SHA-256 before/after. It never changes photographs or generates
human labels. Cosine values are diagnostic measurements, not grouping ground
truth, accuracy estimates, calibrated thresholds, or proof of performance on
3000 independent photographs.
