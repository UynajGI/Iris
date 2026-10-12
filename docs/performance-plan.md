# Selection performance

The selection workflow must remain responsive while importing and analyzing public fixtures. Rust alone does not prevent blocking database locks, serial inference or unbounded image requests.

## Implementation and acceptance

1. Prepare immutable preview jobs under a short database lock, then decode and publish outside it. Bound decoding and background prefetch; prioritize foreground requests and discard canceled queued work. Verify cache invalidation, source changes and shared request leases.
2. Read file dimensions and capture metadata in bounded parallel workers. Commit small batches, including the first batch promptly, and refresh the gallery during import. Preserve cancellation, missing-file handling, scoped scans and existing user decisions.
3. Publish quality scores independently of semantic completion. Bound CPU semantic workers, keep GPU work conservative, and measure the installed model's batch support before claiming a GPU speedup. Preserve source/settings checks and optional-model fallback.
4. Measure the same explicitly configured public portrait corpus before and after: fresh/cached import and opening, first visible photos, preview switching P95, analysis throughput and API/UI responsiveness during analysis. Record hardware, binary/source identity and sample count; generated reports remain ignored.

Correctness gates precede speed claims. A completed HTTP response is not equivalent to a painted browser frame. Distinguish server latency from browser display measurements and warm filesystem cache from fresh application cache. Do not infer model accuracy from successful inference.

Use synthetic fixtures for mutation tests and disposable public-fixture copies where necessary. Never use personal photographs or silently fall back to `test-photos/`.

## Implemented on 2026-10-12

- Cache jobs capture source size/mtime and cache destination under the service lock; image decoding, RAW subprocess waits and atomic cache publishing run outside it. The daemon runs at most two decodes and one background prefetch. Foreground previews outrank queued thumbnails and prefetch. Dropped requests cancel queued work; active RAW subprocesses are terminated cooperatively. Raster decoder calls finish before their result is discarded.
- The frontend limits media requests, reserves a foreground slot, shares leases and promotes queued prefetch when focused. Offscreen thumbnails use an intersection observer. Import commits one initial file then batches of 32; metadata readers use at most eight workers and retain ordered insertion, source checks and cancellation behavior.
- Base quality results publish immediately and in small durable batches. Scores remain usable during semantic grouping; stale settings cannot overwrite current scores. CPU quality workers respect the configured limit and reserve CPU capacity; semantic workers are capped at four. GPU work remains one worker. Startup reports actual runtime selection, allowing full-runtime GPU fallback to expand to CPU workers while preserving analysis settings and cache identity.
- Ordinary name-sorted gallery pages apply SQL limits before deserializing analyses. Other filters/sorts keep their existing behavior. OpenAPI and frontend declarations include stage readiness and the prefetch query.

## Local acceptance

Windows, AMD Ryzen 9 7940HS, 16 logical CPUs, GNU release build. Baseline source is `7b7a11a0aad475c319324a064d46f697605d7263`; the new build contains the local performance changes. Each report records binary SHA-256 and fixture-manifest SHA-256. The corpus is 541 distinct PPR JPEG portraits, 90 source groups, about 1024 pixels on the long edge. Sources are hash-checked before and after; all writes use disposable copies. These results do not measure RAW decoding or full-resolution camera files.

Measurements preceded the beta3 version-metadata bump and source commit. They validate the local performance implementation, not the final release binary or remote release CI; use the recorded binary hashes when comparing evidence.

Two runs per build, executed sequentially in baseline/new/new/baseline order after builds/tests stopped. Application databases and caches are new for each run; filesystem cache and laptop power/temperature are uncontrolled. Ranges below are the two observed runs, not confidence intervals.

| Server measurement                                                             | Baseline           | New build          |
| ------------------------------------------------------------------------------ | ------------------ | ------------------ |
| Import all 541                                                                 | 6.18–7.60 s        | 0.81–1.06 s        |
| First 48 available through API                                                 | 746–752 ms         | 147–185 ms         |
| Unchanged rescan                                                               | 173–192 ms         | 58–114 ms          |
| Foreground preview P95 under thumbnail backlog                                 | 378–612 ms         | 43–45 ms           |
| Fresh isolated preview P95, 60 requests/run                                    | 71–141 ms          | 76–84 ms           |
| Cached isolated preview P95                                                    | 7–25 ms            | 8–16 ms            |
| Base analysis, including validation/init/grouping and concurrent polling/marks | 22.5–27.9 photos/s | 28.7–29.0 photos/s |
| Cached analysis                                                                | 1.35–1.41 s        | 1.24–1.26 s        |
| Mark API P95 during analysis                                                   | 140–167 ms         | 147–161 ms         |

Import and preview contention improved clearly in this sample. Isolated decoding and mark latency show no established improvement. Throughput is about 29 photos/s here; these few runs do not establish a general inference speedup. Generated reports: `artifacts/performance-idle-{before,after,after-repeat,before-repeat}.json`. Earlier `performance-final-*` runs overlapped a debug build and are excluded.

The installed dynamic CPU DINO graph accepted batches 1/2/4/8, with medians about 134/269/548/1226 ms: batching did not improve throughput. The installed DirectML graph has a fixed batch dimension of one and rejected larger batches. Device 0/1 batch-one synthetic tensor medians were about 43/77 ms; preprocessing and the full selection workload are excluded. Provider registration does not establish per-kernel GPU execution. Keep batch one until a separately validated graph/workload justifies changing it. Probe evidence is `artifacts/performance-embedding-batches.json`.

A separate sequential CPU DINO pair computed all 10 eligible semantic candidates in both builds and reused all 10 on the second analysis. Grouping time was 4.12 s baseline versus 2.94 s new; total observed job wall time was 21.99 versus 19.10 s. New base scores were available at 16.27 s while grouping continued. This sample has only 10 eligible candidates among 541 photos, so it does not establish large-corpus semantic scaling. Evidence: `artifacts/performance-idle-dino-{before,after}.json`.

Correctness checks: 193 Rust tests passed, 21 explicitly ignored; frontend 104 passed, one skipped; frontend type check/build, API regeneration/comparison, Rust formatting, Python lint and unchanged complexity thresholds passed. Ignored model/media tests were not treated as passes. Tests cover canceled preview work, source/cache invalidation, durable scan batches, canceled scans, atomic analysis writes, early score visibility and startup fallback feedback.

## Remaining limits

### Browser display and runtime fallback

Headless Chrome, 1440×960, the real frontend/API and disposable 541-photo fixture: first 12 decoded thumbnails appeared in 2.24 s on fresh import and 1.06 s when reopening the cached project. Fresh-run switching P95 was 343 ms for first visits and 90 ms for return visits; while actual analysis remained running, 30 switches had P95 117 ms and the verified star update painted in 40 ms. The full analysis interval recorded 12 main-thread tasks above 50 ms, maximum 102 ms. This is an observed remaining source of brief stalls, not a zero-jank result.

The preview fixture intentionally includes only the CPU runtime. Auto device selection now reports `cpu` with the explicit missing-DirectML warning and starts 12 quality workers. All 541 completed without errors in 26.50 s of job-reported time while the browser was active. The cached follow-up reused all 541 in 1.27 s; its switching sample outlasted the cached job, so it is not a sustained concurrent-inference measurement. That second run had switching P95 values 128–261 ms and a 93 ms long task; do not hide this variation by reporting only the best sample. Evidence: `artifacts/performance-browser-accepted-{fresh,cached}.json`.

The earlier missing-runtime diagnostic used one worker and took about 120.6 s; that exposed the scheduling bug. It is diagnostic evidence, not a controlled speedup ratio. Measurements use decoded images followed by two animation frames, not compositor pixel capture. Browser and HTTP timings have separate meanings.

Native WebView2 painting and full-resolution RAW speed require separate acceptance. This corpus has no independent selection-quality labels; no accuracy claim follows from these tests. Installed GPU graphs do not support batching above one. Large semantic-candidate sets and other machines need additional performance evidence. No model licenses, publishing settings or machine-wide settings were changed.
