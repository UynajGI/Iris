# IrisVision offline model assets

Run `python tools/setup-models.py` to restore the pinned files in `manifest.json`.
Downloaded bytes are checked against upstream Git LFS SHA-256 object IDs. The
Rust loader independently checks compiled-in hashes before constructing sessions.
No photograph is sent to a network service. Python is a development download
utility; production inference runs in-process in Rust through `ort`.

Current analysis version: `iris-vision-v6-local-pipeline-2026-10-07`.
The quality formulas below were introduced in v5 and retained in v6.
Sharpness combines mapped Laplacian/FFT at 75:25 before optional NIQE; the
default NIQE share remains 20%. Face quality now measures local crop sharpness,
exposure and preview resolution at 50:30:20, separately from detector confidence.
`score_breakdown` exposes the raw terms, mappings, weights, contributions and
coverage; it is an uncalibrated technical heuristic, and noise can raise its
sharpness signals. Old analyses require full refresh while human decisions are
preserved. See [formula definitions](../docs/scoring-v5.md) and
[current verification](../docs/development-status.md).

Windows builds load the separately packaged Microsoft ONNX Runtime 1.22.0 x64
DLL (12,418,080 bytes), also hash-checked. The setup utility downloads the official
Microsoft release and retains its MIT license and third-party notices. This DLL
is runtime infrastructure, separate from the approximately 7 MB of model weights.
`ORT_DYLIB_PATH` explicitly selects a custom compatible runtime; the user-selected
runtime is not checked against the bundled runtime hash.

| Asset | Actual input / output | Attribution and license |
|---|---|---|
| YuNet 2023mar, 232,589 bytes | BGR float NCHW 640 square; stride 8/16/32 boxes, scores, five points | OpenCV Zoo, Shiqi Yu; MIT, see LICENSE-YuNet.txt |
| Face Landmarker, 4,920,990 bytes | RGB float NHWC 256 square, 0–1; 478 XYZ points in crop pixels, presence logit | Google MediaPipe, community ONNX conversion by FreeHugsForRobots; Apache-2.0 |
| Face blendshapes, 1,880,927 bytes | 146 selected landmark XY pixel coordinates; 52 coefficients | Google MediaPipe, community ONNX conversion by FreeHugsForRobots; Apache-2.0 |
| NIQE pristine statistics, 29,585 bytes JSON | 36-feature pristine mean/covariance and 7-square Gaussian window | BasicSR, Apache-2.0, see LICENSE-BasicSR.txt |

MediaPipe files are pinned to Hugging Face repository commit
`7c463946f31bcb95be863a6dfa4538b235f7248c`, not a moving branch.
The publisher identifies the converted landmark model as Google MediaPipe and
declares Apache-2.0 for this repository. That is the redistribution evidence
available here; the conversion publisher does not provide reproducible conversion
scripts or independent numerical equivalence evidence. Full license text is
retained in LICENSE-MediaPipe.txt.

Blendshape landmark subset/order is from Google's Apache-2.0
[face_blendshapes_graph.cc](https://github.com/google-ai-edge/mediapipe/blob/master/mediapipe/tasks/cc/vision/face_landmarker/face_blendshapes_graph.cc).
The ONNX graph itself subtracts the point mean and normalizes by mean radius.
Indices 9/10 are eyeBlinkLeft/Right; 44/45 are mouthSmileLeft/Right. The system
uses landmark-derived EAR to corroborate blink. The head-pose gate is a geometric
landmark-plane proxy, not a calibrated camera pose estimator. These thresholds
are conservative defaults and need labeled validation before accuracy claims.

The bundled landmark network provides XYZ coordinates and whole-face presence,
**not per-eye/per-landmark visibility**. Blink coefficients and EAR both depend on
the same predicted landmarks, so agreement does not establish that an eye is
unoccluded. In analysis v3, each eye additionally has measured observation gates:
span below 8 preview pixels, a region crossing image boundaries, implausible
geometry, dark/highlight clipping, or insufficient local contrast suppress only
that eye. `left_eye_unreliable_reason` and `right_eye_unreliable_reason` state the
actual measurement that failed; old records default these fields to null.
These gates are not general occlusion recognition: a textured or skin-coloured
obstruction may still pass. Occluded-eye performance is unverified, and no
occlusion probability or invented visibility confidence is emitted.

Analysis v4 (`iris-vision-v4-conservative-ear-fallback-2026-10-06`) corrects the
blendshape failure path: an observable eye with missing/failed blendshapes keeps
its measured EAR but always has an `uncertain` state. EAR alone cannot declare an
eye open or closed, and cannot establish the all-open condition for recommendation.
Warnings explain the unavailable model/head. Existing observability failures still
withhold the eye entirely, retaining the per-eye reason. Rescoring old EAR-only
definite states also converts them to uncertain; it does not recreate withheld eyes.
The analysis version invalidates v3 inference caches. With valid blendshapes, the
classification thresholds, scoring and grouping formulas are unchanged; the result
version changes. This fix adds no general occlusion detection capability.

Analysis v3 also reports explainable composition geometry: confidence/area-weighted
eye midpoints, with an explicitly named face-box fallback, locate the portrait
subject. Both centering and proximity to rule-of-thirds intersections are accepted.
This is a placement heuristic, not aesthetic truth. Composition occupies 25% of
the existing exposure/framing category (5% of the default total); the five main
weights are unchanged. With no eligible face, composition is null and the entire
category uses exposure. Composition cannot independently trigger rejection.
It is recomputed from stored face geometry during rescoring. New fields are
optional for old JSON records, while the v3 analysis version invalidates cached
inference so the per-eye observation gates also execute.

## Corrections to the planning research

The documented “478 points and 52 blendshapes in one forward pass” is not the
actual model topology: landmarks and blendshapes use two sessions after YuNet.
The Heliosoph artifact mentioned by the research is a different 468-point
pipeline, with inconsistent model-card prose; it is not used here.

No verified NIQE ONNX artifact was found. NIQE is a classical statistical
algorithm, so the implementation uses native Rust instead of an unnecessary
ONNX wrapper: two-scale MSCN, AGGD features, and a covariance pseudoinverse.
The pristine statistics are pinned to BasicSR commit
`8d56e3a045f9fb3e1d8872f92ee4a4f07f886b0a`; the original NPZ and converted JSON
hashes are recorded. Conversion changes storage format only. BasicSR's repository
Apache-2.0 license and attribution are retained; no license exception is declared
for its bundled pristine statistics.

`python tools/verify-niqe.py` compared the native result on the public baboon
benchmark: **5.729573884152**, versus published BasicSR **5.7295763** and MATLAB
**5.72957338** (absolute difference from BasicSR **0.000002416**).
NIQE is enabled by default and contributes 20% within the sharpness component;
`enable_niqe` and `niqe_weight` configure it. Mapping the raw NIQE distance into
the composite score remains an explicit heuristic, not a calibrated quality
probability. Images with fewer than two valid 96-square texture blocks return
null with an explanation; constant or tiny images do not receive fake scores.
SCRFD and DINOv3 are excluded from the general bundle and never silently substituted.
DINOv3 has a separate optional offline bundle; see `tools/DINOV3.md`.
HEIC decoding uses the complete `media/` runtime, license and source closure.

## Optional SCRFD-500M KPS provider (v1 scope, weights not bundled)

The v1 design promised a selectable SCRFD-500M detector. The Rust adapter and
settings now support `face_detector: "scrfd_500m"`; the default remains `"yunet"`.
Selecting SCRFD also requires `scrfd_model_sha256`, exactly 64 hexadecimal digits,
and these local files under the selected model directory:

```text
optional/scrfd_500m.onnx
optional/scrfd_500m.metadata.json
```

The JSON contains `model_id: "scrfd_500m_kps"`, `source_url`, `license_url`
(both HTTPS), and a nonempty `license_note`. This records provenance declarations;
**metadata and possession of a file do not establish authorization**. InsightFace's
[official policy](https://github.com/deepinsight/insightface#license) restricts its
public pretrained weights to non-commercial research, including manual downloads.
The SCRFD [source license](https://github.com/deepinsight/insightface/blob/master/detection/scrfd/LICENSE)
is Apache-2.0 and the wider library describes MIT code licensing; neither grants
unrestricted use or redistribution of the weights. Commercial/product use needs
appropriate model rights. The separate `tools/setup_scrfd.py --research-only`
explicitly obtains the pinned official research artifact. General model setup and
portable packaging exclude it; `optional/` remains excluded from the general bundle.

The adapter accepts the official five-keypoint export contract: one float RGB
NCHW input at 640 square, `(pixel - 127.5) / 128`, black bottom/right padding,
and nine outputs in order `score_8/16/32`, `bbox_8/16/32`, `kps_8/16/32`, or the
complete official ordered numeric set `443/468/493`, `446/471/496`, `449/474/499`.
Shapes are `[N,C]` or `[1,N,C]`, with N=12800/3200/800 and C=1/4/10.
Renamed or differently ordered exports are explicitly incompatible with this
adapter until separately inspected. Distances/keypoint offsets are decoded at
strides 8/16/32 with two anchors per location and NMS at IoU 0.4. YuNet retains
its existing preprocessing, thresholds and postprocessing.

Missing files, malformed metadata, wrong SHA-256, incompatible output names,
shapes/types, or invalid values fail explicitly; none select YuNet as a fallback.
The bytes passed to ONNX Runtime are the bytes whose hash was verified. An initial
synthetic inference checks the complete tensor contract before photo processing.
The lightweight capability helper checks file/hash/metadata only: `available`
does not mean the ONNX graph has executed or the license has been granted.
Selected provider and expected weight hash belong to the analysis settings/cache
identity. SCRFD analyses report provider/hash in their warnings; unchanged YuNet
analyses retain the existing schema. The separate v4 EAR fallback correction above
changes only missing/invalid-blendshape eye certainty, in addition to the version.

Weight-free tests cover settings, explicit missing/hash/metadata failures, RGB
normalization, output shape/finite checks, anchor decoding and NMS. The pinned official
research model passed actual Rust CPU inference, independent geometry reconstruction
and paired 100-JPEG workflows. Comparative accuracy remains unvalidated: YuNet found
426 faces and SCRFD 377; counts alone do not establish accuracy. The design's
"high accuracy" option name is not a measured result. See `tools/SCRFD.md`.

The 100 user JPEG fixtures support decoding, inference and latency smoke tests.
They do not meet the proposed 300-photo/60-burst labeled accuracy benchmark.
Human-verified per-face eye labels are still required for precision/recall or
group-choice success claims.

## Historical v3 runtime evidence

`artifacts/sample-profile-v3-optimized-final.json` records the previous v3 release:
100 JPEGs analyzed without errors in **5.010 seconds** including HTTP polling
(internal pipeline **4.914 seconds**), with per-photo P50/P95
**311.592/487.870 ms**. All 100 cached results were reused in **0.322 seconds**,
with **zero model workers**. Source hashes were unchanged. A fresh run of the
previous portable version took 5.021/1.428 seconds; all non-timing analysis
results have identical SHA-256 digests across those two runs. The demonstrated
improvement is cached reuse, about 4.4 times faster; first-pass differences are
within observed run-to-run variation.
The run yielded 426 faces, 100 finite NIQE values and 99 composition measurements;
709 eye states were withheld, 102 open, 34 uncertain and 7 closed. One additional
eye was withheld because its observation region crossed the image boundary.
Verdicts remained 10 recommend and 90 review. These are functional observations,
not accuracy measurements. This sample is near the 20 photos/s design target
(19.96 including polling; 20.35 internally), without a sustained-load acceptance
claim. The historical v2 result below does not establish v3 acceptance.

## Historical v2 runtime evidence

The runs below predate v3 composition and per-eye observation gates. Their
performance and verdict distribution must not be presented as v3 verification.

The daemon integration run (`artifacts/sample-verification.json`, 2026-10-06)
completed all 100 JPEG analyses without failure and preserved source hashes.
It produced 426 detected faces: 852 possible eyes, of which 708 were withheld
as unreliable, 102 open, 35 uncertain, and 7 closed. Verdicts were 10 recommend
and 90 review. NIQE was finite on all 100 images, spanning 2.37084–5.87617.
These are execution/distribution observations, not labeled accuracy estimates.

The development build with four workers took 91.323 seconds (1.095 photos/s),
with per-photo P50/P95 3.477/3.878 seconds under concurrent load. Incremental
analysis reused all 100 results in 2.149 seconds. This development run does
**not** establish the design's 20 photos/s release-performance target.

The corresponding release build run (`artifacts/sample-verification-release.json`)
completed the same 100 photos with four workers in **8.638 seconds**, or
**11.577 photos/s**. Per-photo P50/P95 were **313.054/468.667 ms** and incremental
reuse took **0.537 seconds**. It produced the same 426 faces and NIQE range.
This earlier four-worker run was below the **20 photos/s** design goal.

The v2 eight-process release pipeline (`artifacts/sample-verification-final.json`)
includes hard deadlines and parallel worker initialization. It completed all 100
photos in **4.718 seconds** (**21.20 photos/s**), with P50/P95 **313.652/429.670 ms**;
incremental reuse took **0.481 seconds**. All 100 inputs retained their hashes and
produced the same face/verdict counts. This meets the throughput target on this
machine and sample; a 3000-photo run and labeled accuracy evaluation remain separate.

## Optional FaceOcc visible-face mask

`occlusion_provider: "none"` is the default and adds no mask inference. Selecting
`"faceocc"` requires an explicit 64-hex `occlusion_model_sha256` and an explicit
`occlusion_min_visible_fraction` in `(0, 1]`; no calibrated threshold is supplied.
The local artifacts are `optional/faceocc.onnx` and `optional/faceocc.metadata.json`.
Metadata requires `model_id: "faceocc_visible_face_v1"`, HTTPS `source_url` and
`license_url`, and a nonempty `license_note`. These declarations do not authorize
redistribution. Optional weights are excluded from portable packages.

The pinned experimental graph takes float RGB `[1,3,256,256]` in `[0,1]`, with
ImageNet normalization inside the graph, and returns `[1,1,256,256]` visible-face
logits. Rust loads hash-verified bytes and validates graph shape and execution
even for a photo with no faces. Selected model failures do not fall back to
unmasked eye states. Artifact status `available` covers files/hash/metadata only.

The aligned crop expands the detection box to 1.5 times its longest side and
uses detector-eye roll. Each eye's rectangular ROI uses its corner span: width
1.2 times the span, height 0.5 times the span. It does not depend on eye aperture.
The reported `visible_fraction` counts mask pixels with nonnegative logits;
`mean_probability` is the average sigmoid, not a calibrated eye visibility
probability. Missing/invalid ROI or a fraction below the chosen threshold hides
the eye state. Existing quality gates remain in force. Crop/ROI geometry and
thresholds still require independent natural-photo validation.

See [research and integration evidence](../docs/occlusion-model-options.md).

## CI smoke test without private photographs

After restoring assets, run:

```powershell
python tools/setup-models.py
cargo test -p iris-core bundled_onnx_tensor_smoke -- --ignored --nocapture
```

This test loads all three pinned ONNX sessions and executes patterned synthetic
tensors. It verifies YuNet's 12 output tensors, the landmark model's three output
tensors, and all 52 finite blendshape coefficients. It passed locally in 0.64 s.
It tests model/runtime compatibility and tensor semantics, not face accuracy.

## Grouping semantics

`group_similar` requires temporal proximity plus pHash/aspect/structure agreement.
`group_duplicates` instead finds strict perceptual duplicate **candidates** across
the library without requiring capture timestamps: Hamming distance at most two,
aspect ratio within 0.5%, and 8-square luminance mean/max differences at most
0.015/0.06. Both use candidate indexes and anchor matching to prevent similarity
chain drift. Neither pHash nor low-resolution structure establishes byte equality
or exact decoded-image equality; neither API authorizes deletion. The service
layer excludes linked RAW/JPEG variants before calling these APIs.
