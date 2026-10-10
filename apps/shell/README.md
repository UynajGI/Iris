# Desktop application and hosting

This package contains the TypeScript application, React presentation, and Tauri
desktop host. The current shell includes overview, review, group comparison,
settings, cache, export and update workflows; native directory picking and host
reconnect are implemented, while the interactive native dialog, close protection,
and cross-platform native validation remain follow-up work. See
`../../docs/frontend-implementation.md` for the current boundary.

From the repository root, `make setup`, `make check-web`, `make test-web`, and
`make build-web` prepare, validate and compile the TypeScript application. `src/index.ts` exports `IrisClient`, `IrisStore`,
`CommandSystem`, and `EventBus`; `src/react.ts` exports React hooks used by the
presentation. The Tauri host owns the local daemon and exposes `window.iris` to
the UI; headless CLI and stdio MCP remain separate entry points.

`window.iris.openFolder()` opens the native directory picker and passes the
selection to the project store. Cancellation returns `false` and leaves the
current project intact. For a custom host, `selectProjectFolder` and
`openProjectFolder` accept an injected native invoke bridge. The Tauri command
waits off the UI thread. Native compilation and bridge cancellation/error tests
are verified; interactive picker validation remains separate from these tests.

The client keeps the daemon session token in memory, puts it in HTTP headers and
WebSocket subprotocols, and fetches authenticated images into revocable object
URLs. WebSocket reconnect uses bounded exponential backoff and full state
refresh; periodic polling recovers dropped events. No frontend operation
deletes source images. Quarantine commit requires the exact preview manifest.

Project settings are scoped by project ID. Decisions are serialized, become
visible after server acknowledgement, and use the server's persisted undo
history. Commands ignore text fields, composing input, and repeated keydown
events. Custom bindings can be supplied to `CommandSystem`.

View preferences are saved per canonical project root: filter/sort values,
page size (1–1000), and the existing review-open flag. Page offsets and photo or
group selections reset on reopen. `ViewPreferencesRepository` validates a
versioned field allowlist; malformed, blocked, or full storage falls back to
defaults or its memory copy. Tokens and media URLs are never included. The
desktop reuses the repository through daemon reconnects and restores the active
project through a read-only request, preserving recent-project order.

`openRecentProject(id)` explicitly updates recency and unhides the project;
`hideRecentProject(id)` removes only the recent-list entry. Background progress
reads do neither. `refreshCache()` loads cache status and migration history.
`migrateCache(destination)` records the migration without deleting old cache
files. Select a history entry with `reviewCacheMigration(id)`, then pass that
exact ID to `cleanupPreviousCache(id)` to confirm cleanup. The server rechecks
the recorded files before removing them; unrelated files are retained.

Previous/next commands and automatic advancement after a decision cross page
boundaries while retaining the active filter and sort. Under a pending-only
filter, a decided photo disappears and focus moves to its shifted successor;
an emptied last page returns to the previous page. Navigation and decision
commands share a queue, and late responses cannot replace a newer project or
filter view.

`accept()` retains the whole-project behavior. Pass `{ photo_ids, category }`
to restrict acceptance, or call `acceptVisible(category)` for the currently
loaded page and `acceptSelected(category)` for the selection. Categories are
`all`, `recommend`, and `reject_suggest`. Explicit empty IDs remain an empty
scope; they never expand to the project. The server validates analysis
freshness and preserves existing manual decisions.

`groupSession` holds the active group's ordered full member list, loaded photos,
focus, comparison candidate IDs, and loading state. `openGroup(id)` fetches
members outside the current library page with at most six concurrent reads.
`focusGroupPhoto`, `setComparisonCandidates`, `moveGroupPhoto`, and `moveGroup`
operate independently of library focus. `acceptGroup(category)` uses the entire
active group; `decideGroup(action, true)` advances through its members and then
to the next group. Explicit `groupKeep`, `groupReject`, `groupFlag`, `groupClear`,
`previousGroup`, `nextGroup`, `previousGroupPhoto`, `nextGroupPhoto`, and
`closeGroup` commands can be bound by the presentation. Keyboard decisions and
photo navigation follow the active group focus; closing the group restores
library keyboard behavior. Loading groups consume keyboard commands without
falling back to the library, and Ctrl/Cmd+A does not select the hidden library.
Explicit `dispatch('keep')` and other library commands retain their library
semantics. `useGroupSession(store)` exposes the same state to React; no comparison
layout is supplied.

Group data refreshes after its writes, undo, decision/resync events, reconnect,
or `refreshGroup()`. HTTP polling also refreshes it while WebSocket is down.
Project switches, dirty/pending analysis, active reanalysis, removed groups,
and changed member order invalidate the session and pending member loads.

Saving project settings or applying a profile clears stale local groups and
refetches authoritative project state. `reanalysisRequired` combines persisted
`Project.groups_dirty` and the full-project `pending_analysis` count, so it also
survives reopening and covers photos outside the visible page. Call `reanalyze()`
to start rebuilding; the flag clears only when the server reports the project
is current. Project-bound asynchronous responses carry a view generation guard
and cannot overwrite a newly opened project's settings or quarantine state.

`Photo.analysis_status` is the server's per-photo `missing`, `current`, or `stale`
assessment against the engine version, canonical settings, and currently scanned
source record. Listing does not re-stat every source file; a scan detects source
metadata changes. Historical `Photo.analysis` stays available. Use
`readPhotoAnalysis(photo)`, `store.photoAnalysis(id)`, or `usePhotoAnalysis(store, id)`
for current scoring: stale or unknown-status payloads return null current score,
verdict, and breakdown. The store/hook also resolve active off-page group members.

Generated `FaceQuality` preserves observed crop dimensions, Laplacian sharpness,
exposure measurements, and mapped quality scores. Unavailable or older face
quality remains absent, with `quality_unavailable_reason` when supplied.
`ScoreBreakdown` exposes server component scores, configured/effective weights,
final score contributions, observed/total counts, and missing reasons. Its terms
include raw inputs, mapping descriptions, weights, and category contributions.
The client preserves these fields without recomputing scores or claiming calibrated
accuracy; optional missing observations are not converted to zero.

`Settings.face_detector` selects `yunet` (default) or `scrfd_500m`; the latter
requires a declared `scrfd_model_sha256`. `IrisClient.models(projectId)` and
`IrisStore.detectorModels` expose read-only model status. The store refreshes it
on project open, settings/profile changes, or `refreshDetectorModels()`, rather
than hashing model files during each progress poll. Missing or invalid SCRFD
does not change the selected provider and analysis fails without fallback.
`available` means disk artifacts, declared hash, and metadata passed checks;
it does not establish licensing, graph/output validity, or verified SCRFD
inference/accuracy. No weights are downloaded and no license is accepted.

The same model-status response includes `occlusion_selected` and `occlusion`.
`Settings.occlusion_provider` defaults to `none`, reported as `disabled` rather
than model availability. Enabling `faceocc` requires `occlusion_model_sha256`
and an explicit `occlusion_min_visible_fraction` in `(0, 1]`; there is no
calibrated default threshold. Missing or invalid FaceOcc fails analysis without
silently disabling the requested provider. `available` checks disk artifacts,
hash, and metadata only; it proves neither eye-visibility accuracy nor licensing.

Generated `Face` and `EyeVisibility` types expose independent
`left_eye_visibility` / `right_eye_visibility` readings with visible-face mask
fraction, mean visible-face probability, sampled pixel count, and method. The client preserves null
eye states and unreliable reasons from the server. Optional visibility fields
may be absent with the default provider. Research weights stay outside default
package copying, including FaceOcc in `models/optional` or unlisted locations.

Run the live integration suite against a built daemon by setting
`IRIS_TEST_DAEMON` to its absolute executable path and `IRIS_TEST_PHOTOS` to a
JPG directory before `npm test`. The test copies three photographs into an
isolated temporary project and verifies actual HTTP/WebSocket behavior,
exports, decisions, recovery manifests, settings, and cache operations. The
source fixture directory is read-only. Default model artifacts are copied to
the temporary model directory from repository `models` (or `IRIS_TEST_MODELS`),
excluding optional weights. The flow also verifies model status and missing
SCRFD/FaceOcc failure without fallback, and a persisted v4 record rejected as stale
until real default-model reanalysis supplies a current score breakdown. Without those variables, only this
integration test is explicitly skipped.

Generate API types from the daemon's exported schema with
`npm run generate:api -- ../../path/to/openapi.json`. The generator is
`tools/generate-client.mjs`; generated source carries the contract hash.

The independent `src-tauri` crate keeps desktop dependencies out of the core
workspace. `cargo test --manifest-path src-tauri/Cargo.toml` tests the supervisor.
Set `IRIS_TEST_DAEMON` and append `-- --include-ignored` to include the actual
daemon crash/restart/token-rotation/graceful-shutdown test.
For a desktop build, run `npm run build` then
`cargo build --manifest-path src-tauri/Cargo.toml --features desktop`.
Place `iris-daemon.exe` beside the shell executable, or set `IRIS_DAEMON_PATH`
to an absolute daemon executable path during development. The host launches the
daemon with a private application data directory and a random loopback port,
validates its version, checks authenticated heartbeats, restarts failed children,
and notifies the frontend to retrieve a fresh token. Exit requests graceful
shutdown through stdin before a bounded forced-exit fallback. Update installation
uses a stricter graceful-shutdown path: timeout or abnormal exit cancels the install.
The visible presentation remains separate work.

The host passes an absolute model directory to the daemon. Set `IRIS_MODEL_DIR`
explicitly, use `DaemonHost::with_model_dir`, or install `models` beside the
daemon (`resources/models` is also supported). It never depends on the caller's
working directory. `IRIS_DATA_DIR` optionally overrides the desktop data path.

From the repository root, `tools/package-local.ps1` creates an unsigned Release
portable directory with the shell, daemon, CLI, frontend assets, models, ONNX
runtime, license texts, and a SHA-256 file manifest. Add `-SkipBuild` when all
Release artifacts are already built. Existing output directories are never
overwritten. `-OutputDirectory` chooses a new location.
Only default manifest artifacts and their license files are copied. Optional
SCRFD weights under `models/optional` and unlisted model files are excluded
before copying; `Verify.ps1` rejects packages containing optional weights.

The packaged `Verify.ps1` checks hashes, launches the packaged shell host from
an unrelated temporary working directory without model environment overrides,
then scans and analyzes a generated JPEG through the packaged daemon. It writes
`verification.json` without credentials or user photographs. `Launch.ps1`
resolves its own location and starts the desktop interface; `iris-cli.exe`
provides headless workflows independently of the interface.

The native updater exposes explicit check, download and install operations through
`UpdaterStore`, `useUpdater`, and `window.iris.updater`. Portable builds report
`not_configured`; production trust is compiled into an NSIS build. See
[the native contract](../../docs/updates.md) and
[installer/signing tools](../../docs/native-updates.md). The shell's desktop
dependencies are included when collecting package licenses.

`native-smoke` is a separate test feature. From the repository root, build it and
run `tools/verify-native-title.ps1` to load the real hidden WebView, verify frontend
initialization, language/title IPC and the default updater state. Production
`desktop` builds do not contain these test commands or scripts. This does not
automate directory-dialog clicks or verify a Windows installation/upgrade.

<a id="module-map"></a>

## Module map

The application is split into transport, state, presentation, and native hosting
layers:

| Area | Files | Responsibility |
| :--- | :--- | :--- |
| Client and transport | `client.ts`, `transport.ts`, `types.ts`, `generated/api.d.ts` | Authenticated HTTP/WS calls and generated contract types |
| State and workflows | `store.ts`, `group-session.ts`, `commands.ts`, `events.ts` | Project state, queued decisions, groups, keyboard commands and resync |
| Analysis and preferences | `analysis-readout.ts`, `view-preferences.ts`, `preferences.ts`, `i18n.ts` | Score display, durable view settings and localized labels |
| React entry | `main.tsx`, `react.ts`, `index.ts` | React mounting, hooks, and public application exports |
| UI screens | `ui/App.tsx`, `ui/LibraryNav.tsx`, `ui/TaskBar.tsx`, `ui/PhotoFilters.tsx`, `ui/MarkTools.tsx`, `ui/ExecutionDetails.tsx` | Library, review, filters, decisions and task status |
| Settings and files | `ui/AnalysisSettings.tsx`, `ui/CacheSettings.tsx`, `ui/FileWorkflows.tsx`, `ui/OptionalModels.tsx`, `ui/ModelTaskBar.tsx` | Analysis, cache, export/recovery and model workflows |
| Shared UI | `ui/components.tsx`, `ui/Icon.tsx`, `ui/app.css`, `ui/tokens.css`, `ui/icons/` | Reusable controls, styling tokens and licensed icons |
| Tauri host | `desktop.ts`, `native-dialog.ts`, `native-title.ts`, `updater.ts` | Daemon lifecycle, native dialogs/title and update bridge |

The map is an orientation aid; behavior and API contracts remain defined by the
daemon and shared state layer. Individual source files do not each need a README.
