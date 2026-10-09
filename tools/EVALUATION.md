# Independent model evaluation

These commands use only the Python standard library and read the project SQLite database without modifying it. They do not run inference or label photographs automatically.

```powershell
python tools/evaluate.py prepare --database .iris/library.sqlite3 --project 1 --output artifacts/annotations.json
python tools/evaluate.py evaluate --database .iris/library.sqlite3 --project 1 --annotations artifacts/annotations.json --output artifacts/evaluation.json
```

Preparation creates every photo and group with `annotation_complete: false` and empty human labels. Independently inspect each photograph, add the truth, and mark completion. `--include-suggestions` optionally exports predicted boxes under `suggested_face_boxes`; these remain separate from `ground_truth` and are not accepted as labels. Evaluation refuses incomplete or missing annotations, missing analyses, changed photo fingerprints, or changed group membership. Output paths must be new files so existing human annotations are never overwritten.

Each human face uses a bounding box normalized to the EXIF-orientation-corrected image, and anatomical left/right eye labels:

```json
{
  "annotation_complete": true,
  "ground_truth": {
    "faces": [
      {"bbox": [0.10, 0.15, 0.20, 0.25], "left_eye": "open", "right_eye": "closed"}
    ]
  }
}
```

Eye labels are `open`, `closed`, or `ungradable`. A completed empty face list explicitly means no human faces. For every group, independently set `ground_truth.acceptable_photo_ids` to all acceptable recommendations and complete the record; an empty reviewed list means none are acceptable. Group labels use the group kind plus stable member sets, so `burst` and `duplicate` can contain the same photos without sharing labels. Model reranking and regenerated database group IDs do not force relabeling. Changed group membership does require review.

An `ungradable` eye may additionally carry `left_eye_ungradable_reason` or
`right_eye_ungradable_reason`: `occlusion`, `small_face`, `pose`, `lighting`,
`blur`, or `other`. Older labels without a reason remain valid and use
`unspecified`. These reasons must be supplied by the independent annotator.
The `ungradable_eyes` report counts determinate, uncertain, and hidden model
states by reason; it reports missed faces separately. Display rates use only
matched human faces as the denominator so missed faces cannot make suppression
look better. This measures whether a state was exposed on an ungradable eye,
not whether the unknown open/closed state was correct. Ordinary eye accuracy
still excludes ungradable truth; the new report keeps that exclusion from hiding
occlusion-related display failures.

The report contains face detection precision/recall using one-to-one, maximum-cardinality IoU matching with maximum summed IoU as tie-breaker; `--iou-threshold` defaults to 0.5. Eye accuracy is measured only for answered, matched, human-gradable eyes. Coverage and end-to-end correct rate also include eyes on undetected faces; null/unreliable predictions and `uncertain` predictions count as explicit abstentions. Ungradable human eyes are excluded and reported separately. False-positive faces are counted in detection precision, not assigned invented eye truth.

Group success compares the current first-ranked member with independently supplied acceptable IDs. P50/P95 use linear interpolation over available stored `elapsed_ms`, and are not whole-pipeline throughput. Zero denominators return null rather than implying perfect accuracy.

Add `--require-design-gate` to require at least 300 photos and **60 unique
`kind: "burst"` member sets**. Member order does not affect uniqueness.
Every stored group must contain at least two distinct photos; singleton groups
are refused rather than treated as trivial successful recommendations.
`duplicate` groups never contribute to this threshold, including duplicate
groups with the same members as a burst. The report exposes
`unique_burst_groups`, `unique_duplicate_groups`, and `minimum_burst_groups`;
the older `minimum_groups` field remains an alias for the minimum **burst** count.
Repeated stored groups of the same kind are still refused by annotation
validation rather than counted as extra evaluation samples.

This is an optional dataset-size check. It cannot prove that stored groups are
real consecutive captures, that different groups represent independent bursts,
or that annotations are independent. It is neither an accuracy threshold nor
certification. The current 100-photo smoke dataset may be evaluated after human
labeling without that flag; its predictions do not constitute labels, and current
empirical accuracy remains unverified.

## Optional v5 quality preferences and term ablations

The same `prepare` / `evaluate` commands support an additive optional
`quality_preferences` list in annotation schema version 1. Existing annotation
files remain valid. Preparation creates an **empty** list; it does not select
pairs, rank photographs, or manufacture quality truth. With no quality pairs,
the report says `quality_preferences.status: "not_requested"` and returns empty
metrics, not an apparent 0% result.

After independently inspecting photographs, an annotator may add records with
this structure. The following is an **incomplete template**, not a label:

```json
{
  "id": "sharpness-pair-1",
  "dimension": "sharpness",
  "a": {"photo_id": 1},
  "b": {"photo_id": 2},
  "annotation_complete": false,
  "ground_truth": {"preference": null}
}
```

Use `sharpness` to compare the visible sharpness of two whole photographs. For
local face technical quality, use `dimension: "face_quality"` and endpoints
such as `{"photo_id": 1, "face_index": 0}`. `face_index` is a zero-based index
into that photograph's **human** `ground_truth.faces`, never a detection index.
The existing one-to-one IoU matching identifies the corresponding prediction;
missed faces or missing supported quality observations cause abstention.
Keep these references consistent if the human face list is edited. Two different
human faces in the same photograph may be compared.

Complete a record only after human review, setting `preference` to `a`, `b`,
`tie` (equally good), or `ungradable` (cannot judge). Ties are labels, not missing
data. Self-comparisons, duplicate IDs, repeated pairs including reversed
endpoints within the same dimension, missing photos, and invalid human-face
indices are rejected. Base photo/group annotations and fingerprints must still
pass all existing checks. Suggestions, detector confidence, model masks, and
model rankings must not be copied into human preference truth.

The quality report evaluates supported stored v5 measurements without new
inference or changing settings:

| Dimension | Full score | Leave-one-term-out variants |
|---|---|---|
| Sharpness | Stored sharpness component | Without Laplacian, FFT, or NIQE |
| Face technical quality | Matched face's stored local quality | Without local sharpness, exposure, or resolution |

For each variant, the remaining **stored internal weights** are renormalized to
sum to one. This is an explicit diagnostic ablation, not the production formula
or a fitted alternative; for example, removing FFT from 60/20/20 leaves
Laplacian/NIQE at 75/25. Removing an already absent or zero-weight term leaves
the score unchanged. If no positive-weight observed terms remain, the variant
abstains. The evaluator checks numeric bounds, weight sums, and consistency
between full scores and weighted terms. Unsupported versions/methods or missing
observations abstain rather than acquiring invented values.

Predicted preferences use higher scores as better; differences at most `1e-9`
score points are predicted ties. A human tie is correct only for a predicted tie;
there is no automatic half-credit. Ungradable pairs are counted separately and
excluded from agreement denominators. Each variant reports answered/abstained
counts, coverage, agreement among answered pairs, and end-to-end agreement over
all gradable pairs. Zero denominators are `null`. The difference from the full
score uses only pairs answered by **both** variants and exposes that paired
denominator, preventing different missing-observation coverage from appearing
as a ranking improvement. Per-pair scores and predictions are included for audit.

No verdicts or group rankings are recomputed by these ablations. Results reflect
only independently supplied pairs; the tool cannot certify their independence,
representativeness, or lack of selection bias. Quality agreement on this sample
does not calibrate coefficients, establish causality, or prove general focus or
portrait preference accuracy. The existing local v5 templates still contain no
completed human quality labels, so no such accuracy result is currently available.

Run the synthetic, known-answer evaluator tests with:

```powershell
python -m unittest discover -s tools/tests -p test_evaluate.py -v
```

The local v3 run also prepared a consistent SQLite backup at
`artifacts/evaluation-v3-final.sqlite3` (project 1) and an independent annotation
template at `artifacts/annotations-v3.json`: 100 photos and 19 generated groups,
all incomplete with empty human truth. The backup passes SQLite integrity checks.
An attempted evaluation correctly refused the incomplete first photo and created
no accuracy report. Keep the referenced `test-photos` fixtures in place while
labeling; source fingerprints are checked before evaluation.

After completing the human labels, evaluate this exact snapshot with:

```powershell
python tools/evaluate.py evaluate --database artifacts/evaluation-v3-final.sqlite3 --project 1 --annotations artifacts/annotations-v3.json --output artifacts/evaluation-v3-labeled.json
```

To preserve a fresh snapshot during future functional verification, pass
`--snapshot <new-output.sqlite3>` to `tools/verify_sample.py`. It uses SQLite's
online backup API, explicitly closes both connections, and refuses to overwrite
an existing snapshot. Model suggestions are never promoted to human truth.

`verify_faceocc.py --threshold 0.9 --output-dir artifacts/<new-directory>` runs
paired default/optional functional verification using the pinned local research
ONNX. It preserves separate `default.sqlite3` and `faceocc.sqlite3` snapshots;
both use project 1. The threshold is an explicit experiment, not calibrated.
The 2026-10-07 run in `artifacts/faceocc-integration-v2/` records 11 additional
hidden eyes without assigning correctness. Evaluate those predictions only
against independently completed labels, including normal closed eyes and
ungradable eyes with occlusion reasons. Mask outputs are never label sources.

The historical v5 snapshot is `artifacts/evaluation-v5-observed-quality.sqlite3`
(project 1); `artifacts/annotations-v5.json` is its independently completable
template without model suggestions. All labels remain incomplete, and an
attempted evaluation correctly returned exit code 2 without an accuracy report.
Use this snapshot only to evaluate v5 scoring; current validation boundaries are in [the validation overview](../docs/validation.md). `v5-scoring-regression.json` records
score movement with unchanged raw observations; it is not a labeled evaluation.
