# Local API contract

All REST paths start with `/api/v1`. JSON bodies and responses; errors use
`{"error":"..."}` with a non-2xx status. All routes require the session bearer
token, including bootstrap and media. The daemon binds IPv4 loopback, default
random port, and prints one startup JSON line `{base_url,token,version}` to stdout.
Credentials are never served by an unauthenticated HTTP endpoint.

`GET /api/v1/devices/gpu` returns `status`, `source`, `default_device_id`,
`adapters`, `inference_verified` and `reason`. Status is `available`,
`unavailable` (DXGI failure) or `unsupported` (non-Windows). Enumeration failures
are a typed inventory result, not an empty success. Each adapter's `device_id`
maps directly to `AnalysisSettings.directml_device_id`; preserve DXGI order and
do not renumber filtered software/remote entries. Reboot/hot-plug can change it.
Memory values are capacities, not usage; LUIDs are boot-local. Enumeration never
loads ORT or models, so `inference_verified` is always false. See
[GPU contract and validation](gpu-scale-validation.md) for full semantics.

Daemon startup accepts `--worker-limit 1..16` (default 12). Each project analysis
pool is bounded by that limit, available logical CPUs and pending photos; valid
all-cached requests still create no workers. This startup resource policy is not
part of REST analysis settings or their cache fingerprint. Concurrent projects
have separate pools; the flag is not a daemon-wide process or memory cap.

Project responses include persistent `groups_dirty` and whole-project
`pending_analysis` (active photos without current-engine analysis). Either signals
that analysis/grouping needs refresh; an empty group list alone does not.
Clients must refresh these fields after jobs/settings changes and guard late
responses against project switches. Saving settings does not automatically start
inference; the application exposes an explicit reanalysis action.

WebSocket `/api/v1/events` uses subprotocols `iris` and `iris-token.<token>`;
server selects `iris`. Events are `{event, project_id, data}`. Event families:
`scan:progress`, `analysis:stage`, `verdict:updated`, `session:changed`.

| Method | Path | Input |
| :--- | :--- | :--- |
| GET | /bootstrap | capabilities and version |
| GET | /devices/gpu | DXGI device indices/names/capacities; no project or models required |
| GET | /projects | recent projects |
| POST | /projects | `{root}` |
| GET | /projects/{id} | project details |
| POST | /projects/{id}/scan | start scan |
| POST | /projects/{id}/analyze | start/resume incremental analysis |
| GET | /projects/{id}/progress | current job |
| POST | /projects/{id}/cancel | cancel current job |
| POST | /projects/{id}/pause | pause analysis |
| POST | /projects/{id}/resume | resume analysis |
| GET | /projects/{id}/photos | query filters defined by PhotoFilter |
| GET | /photos/{id} | photo details |
| GET | /photos/{id}/thumb | JPEG thumbnail |
| GET | /photos/{id}/preview | lazy JPEG preview |
| GET | /photos/{id}/original | authenticated original bytes for explicit full-resolution review |
| GET | /projects/{id}/groups | similar groups |
| POST | /projects/{id}/decisions | `{photo_ids,action,link_variants}` |
| POST | /projects/{id}/undo | undo last batch |
| POST | /projects/{id}/accept | adopt model suggestions (marks only) |
| POST | /projects/{id}/export/xmp | `{scope,overwrite}`; duplicate selected sidecar destinations return 400 before writes, in either overwrite mode |
| POST | /projects/{id}/export/copy | `{destination,scope}` |
| POST | /projects/{id}/export/csv | `{destination}` |
| POST | /projects/{id}/import/csv | `{source}` |
| POST | /projects/{id}/quarantine/preview | create confirmation manifest |
| GET | /projects/{id}/quarantine | persisted manifests and interrupted moves for recovery |
| POST | /projects/{id}/quarantine/commit | `{manifest_id}` |
| POST | /projects/{id}/quarantine/restore | `{manifest_id}` |
| GET, PUT | /settings?project_id={id} | persisted per-project analysis settings |
| GET, POST | /profiles | list/save profile |
| DELETE | /profiles/{name} | delete profile |
| POST | /profiles/{name}/apply | `{project_id}` |
| POST | /projects/{id}/profiles/{name}/estimate | estimate verdict changes |
| GET | /projects/{id}/cache | cache statistics |
| POST | /projects/{id}/cache/cleanup | clear rebuildable caches |
| POST | /projects/{id}/cache/migrate | `{destination}` |
| GET | /openapi.json | generated schema |

UI/UX is intentionally deferred to the user. L4 exports functions, state and
React hooks that a later presentation layer can consume.

`bootstrap.recovery_notice` exposes a migration/recovery notice when applicable.
`Photo.analysis` carries the concrete `VisionAnalysis` schema. Face boxes and
landmarks use pixels in the EXIF-oriented analysis image (`analysis.width` /
`analysis.height`); scale those coordinates to the rendered oriented preview.
`original_width` / `original_height` retain the source JPEG's pre-orientation
dimensions. The `/original` route streams untouched source bytes and never uses
the bounded analysis decoder.

Group `kind: duplicate` means a conservatively corroborated visual duplicate
candidate, not proven byte equality. Neither group kind changes decisions or
moves files. Model failures leave completed per-photo work intact and set job
state `failed`; callers must inspect `errors` and may resume incrementally.

Scan terminal progress is synchronized with the final `ScanReport`, including
errors/skips after the last successful-file callback. Nonempty errors yield
`failed`; explicit cancellation takes precedence as `cancelled`. Scan completed
counts are `added + changed + unchanged + skipped`; directory/file errors are
reported separately, and terminal total equals that completed count. Partial
imports remain available even when the scan fails.

Model status retains detector `selected`/`detectors` and adds
`occlusion_selected`/`occlusion`. The default occlusion provider is `none`
(`disabled`); `faceocc` requires an explicit model SHA-256 and minimum visible
fraction. `available` verifies local artifact/hash/metadata only. Selected
artifact failures are checked before cache reuse. Provider/hash/threshold changes
invalidate features and require reanalysis; old settings normalize to disabled.

Optional `Face.left_eye_visibility`/`right_eye_visibility` contain the visible-face
mask fraction, mean sigmoid, sampled-pixel count and method. These measurements
can coexist with a hidden eye state and do not imply calibrated eye visibility.
Missing readings must not be interpreted as zero occlusion. Existing eye-state
quality gates remain authoritative; see [model notes](../models/README.md).

`Photo.analysis_status` is `missing`, `current` or `stale`, based on the stored
analysis version and normalized project settings. A scan that detects source
changes removes the old analysis. This marker does not continuously inspect
external file bytes. Historical analysis remains readable, but stale values
cannot be presented as current scores or accepted as current suggestions.
Verdict-filtered lists and score/suggestion ordering use only current analysis;
stale records retain their historical payload but have no current sorting score.
Rebuilt groups likewise exclude stale analysis. After partial reanalysis, a
valid current subset may have `groups_dirty: false` while `pending_analysis`
still identifies photos requiring work.

v5 adds optional `Face.quality`/`quality_unavailable_reason` and
`VisionAnalysis.score_breakdown`. Components expose configured and normalized
weights, mapped scores, contributions, coverage counts, raw terms and missing
reasons. Contributions sum to the composite score; these are technical
heuristics, not calibrated quality probabilities. Profile estimates exclude
missing/stale records, and report required refresh even if settings did not
change. The settings diff includes removed optional fields, so estimates match
the invalidation performed when applying the profile. Full version reanalysis
preserves human decisions; see [formulas](scoring-v5.md).

The headless store serializes settings writes and profile applications in command
order. Once a write succeeds it marks cached analysis historical before fetching
fresh state, including when the refresh fails. Library decisions refresh the
complete active group as well as the current page, including linked variants.
