# Model quality: implementation and remaining acceptance gaps

Version scope: the v5 scoring work below is retained in the current v6 pipeline. The v5 version identifiers are historical, not the current cache identity. Current implementation and unvalidated quality claims are separated in [HANDOFF](HANDOFF.md).

Updated 2026-10-07. The v5 implementation addresses the two former computation
gaps below. It does not establish photographic preference accuracy. Formulas,
constants, missing-observation behavior and limitations are specified in
[scoring-v5.md](scoring-v5.md).

## FFT now participates in sharpness scoring

Previously, FFT energy was measured and stored but never read by rescore.
v5 combines mapped Laplacian and FFT scores at 75:25 before optional NIQE.
With the default NIQE weight of 0.2, the effective shares within sharpness are
60% Laplacian, 20% FFT and 20% NIQE. Raw inputs, mappings, internal weights and
normalized final contributions are returned in score_breakdown.

These coefficients are explicit heuristic choices, not fitted human preference
parameters. Controlled blur checks verify response on their fixtures. Noise can
raise both Laplacian and high-frequency energy: the fixed grey-noise fixture
raises the combined pre-NIQE score from zero for flat grey to about 98.54.
The implementation therefore makes no noise-rejection or universal focus claim.
Independent sharpness labels and an ablation report are still required to judge
ranking quality or select better coefficients.

The evaluator now supports independently supplied sharpness and local face
quality pair preferences, with full and leave-one-term-out weighted comparisons.
Missing observations abstain, and paired deltas use the same answered pairs.
This diagnostic operates on stored v5 observations; it does not rerun inference
or change production scores. No completed human preference labels or resulting
accuracy report are available. See [EVALUATION.md](../tools/EVALUATION.md).

## Face quality now uses local observations

Previously, the face-quality category averaged detector confidence. v5 keeps
that confidence as detection evidence and instead measures a clipped, unscaled
preview face crop. Local Laplacian, exposure and crop resolution map to a
technical score at 50:30:20. Invalid or tiny crops return a missing reason;
they are not replaced with detector confidence or fabricated observations.
The category reports observed and total face counts.

This is technical crop quality, not a learned portrait-aesthetic score. Head pose,
eye state and smile remain separate observations. Background inside the box,
texture, noise, preview scaling and different face sizes can affect the score.
Preference quality on small faces, side views, low light and multiple subjects
still needs independent labels. The five default category weights are unchanged.

## Versioning and evidence boundaries

The analysis version changes to v5. Old records remain readable but require
feature reanalysis; rescoring cannot manufacture missing local observations or
upgrade a record's version. The application exposes per-photo freshness and
excludes stale results from current suggestion adoption and profile estimates.
Human decisions survive reanalysis.

Tests and unlabelled JPEG runs establish execution, formula arithmetic and
selected invariants. Score/verdict movement is recorded separately from unchanged
detection and eye measurements. It is not selection-accuracy evidence.

Optional FaceOcc now runs in Rust, but its ROI geometry and explicit threshold
are not calibrated against natural occlusion labels; its measured extra cost
also exceeds the 20-photo/s budget on the local 100-image sample. SCRFD now has a
pinned official non-commercial research artifact, real Rust CPU inference,
independent geometry checks and a 100-image paired run. Comparative accuracy is
still unvalidated. DINOv3 embeddings and semantic grouping also run locally;
their thresholds and grouping quality have not been calibrated with human labels.
Human annotation is deferred by the user's current scope; see [HANDOFF.md](HANDOFF.md).
The 300-photo/60-burst labeled acceptance benchmark remains incomplete.
The structural gate now counts only distinct burst member sets with at least two
photos; duplicate groups cannot satisfy it. It cannot establish that the groups
are real independent bursts or that labels are independently produced.
See [EVALUATION.md](../tools/EVALUATION.md) for the independent-label workflow;
never generate its truth from the predictions being evaluated.
