# Optional SCRFD research workflow

The default detector remains YuNet. `scrfd_500m` selects the original detector
from the official InsightFace `v0.7` `buffalo_sc.zip` release. It is a separately
configured local option, not a demonstrated accuracy upgrade.

## Model permission and provenance

The official model policy at revision
`3e6486942a1be2da0e5b475fac375ea73264bd21` explicitly restricts pretrained weights
to **non-commercial research purposes only**, including manual downloads:
[model terms](https://github.com/deepinsight/insightface/blob/3e6486942a1be2da0e5b475fac375ea73264bd21/python-package/README.md#license).
The code's MIT license does not extend commercial permission to these weights.
No account or acceptance gate was bypassed. No commercial authorization has been
established. Commercial use requires an applicable separate grant/model.

`tools/setup_scrfd.py` requires the explicit `--research-only` workflow and places
the detector only in ignored `models/optional/`. It retains model-use terms,
source provenance and hashes, refuses to overwrite different local content, and
does not modify default settings or the distribution manifest. The upstream
archive also contains a face-recognition model; that member is not installed or
used. Existing package scripts exclude `models/optional`.

```powershell
make models-scrfd ARGS=--research-only
# Or verify an already downloaded archive with the same pinned hashes:
make models-scrfd ARGS="--research-only --archive artifacts/scrfd-research/buffalo_sc.zip"
```

| Artifact | SHA-256 |
| :--- | :--- |
| Official `buffalo_sc.zip` (14,969,382 bytes) | `57d31b56b6ffa911c8a73cfc1707c73cab76efe7f13b675a05223bf42de47c72` |
| Original `det_500m.onnx` (2,524,817 bytes) | `5e4447f50245bbd7966bd6c0fa52938c61474a04ec7def48753668a9d8b4ea3a` |

These are locally measured pins of the official HTTPS release bytes, not a
publisher signature. Model bytes are not converted or renamed internally.
The local filename is `scrfd_500m.onnx`.

## Tensor and image geometry

The float32 input is `[1,3,640,640]` in RGB order, normalized as
`(x - 127.5) / 128`, with black bottom/right padding. The original graph accepts
dynamic spatial dimensions; this adapter deliberately fixes inference to 640.
The nine official output names, in order, are:

```text
scores: 443 468 493
boxes:  446 471 496
KPS:    449 474 499
```

They correspond to stride 8/16/32, two anchors per cell, and 1/4/10 channels.
At 640 input the counts are 12800/3200/800. The adapter also retains its prior
explicit `score_*`, `bbox_*`, `kps_*` schema. Mixed, shuffled or unknown schemas
are rejected. Every output's shape, finiteness and value constraints are checked
even for low-confidence anchors.

The original regression heads can produce finite negative box distances, even
on blank input. These are accepted as signed regression values, as in upstream;
non-finite tensors and degenerate decoded boxes remain rejected/omitted. A blanket
non-negative-distance check would incorrectly reject the real published model.

Coordinates restore the resized-height scale, with no half-cell anchor offset.
The product uses Triangle resizing, clips boxes to the preview, filters boxes
smaller than 8 pixels and uses continuous-coordinate IoU NMS at 0.4. Upstream
uses OpenCV linear resizing and pixel-inclusive NMS. This integration therefore
does not claim bitwise equivalence to upstream end-to-end predictions.

## Local verification

No inference uploads photographs. Run the ignored Rust model test only after
explicitly acquiring the research weight:

```powershell
cargo test -p iris-core research_official_scrfd_onnx_and_optional_geometry_capture -- --ignored --nocapture
```

For a private photograph's raw-head geometry capture, set
`IRIS_SCRFD_GEOMETRY_PHOTO` to an existing local JPG and
`IRIS_SCRFD_GEOMETRY_REPORT` to a new JSON path under ignored `artifacts/`, then
run that test. It creates the output without replacing earlier evidence.

```powershell
python tools/verify_scrfd.py --geometry artifacts/scrfd-research/geometry.json --geometry-only --output-dir artifacts/scrfd-geometry-verification
python tools/verify_scrfd.py --daemon target/release/iris-daemon.exe --output-dir artifacts/scrfd-paired-final
```

The geometry checker independently reconstructs box/keypoint coordinates from
captured raw heads (absolute tolerance 0.002); an empty detection fixture cannot
pass as positive geometry evidence. The paired workflow uses the same private
100 JPGs for both detector settings, checks source hashes before/after, preserves
SQLite snapshots, tests cache reuse and existing domain operations, and verifies
that non-detector measurements agree. It reports detection/eye counts and timing.

Neither synthetic tests nor paired unlabelled runs establish precision, recall,
better group recommendations, eye-state correctness or portrait preference.
Those require independent annotations. Do not generate truth from either
detector's predictions. Temporary or optional research artifacts are not general
distribution deliverables.

## Verified local checkpoint (2026-10-07)

* Six SCRFD Rust unit tests passed; the separately invoked ignored test passed
  using the original model and the project's bundled ONNX Runtime. Square,
  landscape and portrait synthetic inputs all executed successfully.
* A real private JPG's two detected faces were captured as raw heads and decoded
  geometry. Independent coordinate reconstruction matched both, with maximum
  error `0.0000201157427` against tolerance `0.002`.
* The release paired workflow completed both sets of 100 JPGs with no failures,
  full cache reuse, unchanged source hashes and identical non-detector metrics.
  YuNet returned 426 faces and SCRFD 377: 69 images had equal counts, 31 had fewer
  SCRFD detections, and none had more. These counts do not measure accuracy.
* Default YuNet predictions matched all 100 records in
  `artifacts/evaluation-v5-workers-final.sqlite3` exactly after excluding only
  `elapsed_ms`. No settings or prediction fields were otherwise normalized.
* Observed elapsed times were recorded while other CPU work could overlap;
  this run does not support a relative-speed conclusion.

Evidence: `artifacts/scrfd-paired-final/verification.json`,
`artifacts/scrfd-paired-final/default-regression.json`, and
`artifacts/scrfd-geometry-verification/geometry-verification.json`.
Release daemon SHA-256:
`bc1fde063aa5696694c1ba4b5afc35d6fa6602f82fe8b718264b31afb5937084`.
CLI SHA-256:
`fafb348b82ac72f1fc45a42da3a261bae343e95f378709dc63b9f10d17002a45`.
