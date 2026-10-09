# Native installer updates

Iris exposes a native check → download → install flow. The default portable
build reports `not_configured` (`installer_required`) and makes no update network
request. NSIS builds without release trust configuration also report
`not_configured` (`release_configuration_missing`). No update check or installation
runs automatically at startup. UI presentation remains separate.

For the Windows installer scripts, signing inputs and release boundaries, see
[native-updates.md](native-updates.md).

## Release configuration

These environment values are embedded at **build time**, never read as runtime
overrides:

| Variable | Value |
| :--- | :--- |
| `IRIS_DISTRIBUTION` | `nsis` for an installer build; absent or `portable` disables installer updates |
| `IRIS_UPDATE_ENDPOINT` | Trusted HTTPS manifest endpoint, without URL credentials or a fragment |
| `IRIS_UPDATE_PUBLIC_KEY` | Contents of the Tauri public key file (base64), not its path |

Endpoint and key must be supplied together, only for NSIS. Partial configuration
fails the build. Invalid distribution, URL or key fails desktop initialization
explicitly. All three variables invalidate Cargo's build-script cache when changed.
Use a separate installer build profile/target directory so enabled installer
binaries cannot replace portable artifacts accidentally.

The native updater requires HTTPS for manifest and artifact requests, including
redirects. It uses Tauri's official updater signature verification and requires a
signed release version (`requireSignedVersion`). The signature's trusted comment
must contain `version:<version>` as a tab-separated field. The manifest's version
must be newer than the running application and agree with the signed version.
Release signing must therefore use a CLI/signing workflow that includes this field.
Authenticode code signing and Tauri updater signatures are separate operations.

## Native contract

Only the main window may invoke these commands. They take **no application-supplied
arguments**: `update_status`, `check_for_update`, `download_update`, `install_update`.
The webview cannot choose an endpoint, public key, artifact path, installer argument,
or artifact bytes. The updater plugin's own IPC commands are not granted by the
window capability.

Each successful command returns `UpdateStatus`; `updater:status` emits the same
object during work. States are `not_configured`, `idle`, `checking`, `up_to_date`,
`available`, `downloading`, `downloaded`, `installing`, and `failed`. Metadata contains
current/new version, unrendered Markdown `notes`, and optional Unix publication time.
Progress contains `downloaded_bytes` and nullable `total_bytes`. Errors use
`{code, message}`. Overlapping native operations reject with `busy`.

`download_update` waits for both transfer and signature verification. Only that
successful return creates an in-memory installable artifact bound to the checked
update. Transfer completion alone does not make it installable. `install_update`
requires a separate explicit call. On Windows, successful installer launch exits
the application; it does not return an application-level completion receipt.

Before installation, the host pauses supervision and refuses session/start
requests, then asks the daemon to stop gracefully. A 15-second timeout cancels the
installation instead of installing over a force-killed daemon. An abnormal daemon
exit also cancels installation. Installation failure
resumes supervision, restarts the daemon, and emits `daemon:restarted` so consumers
can obtain a fresh session token. Recovery failure is explicit. The Windows
pre-exit hook does not destroy webview resources, because installer launch can
still fail after that hook.

## Verification

Run native protocol, signature, state and host unit tests without Wry/dialog:

```powershell
cargo test --manifest-path apps/shell/src-tauri/Cargo.toml --features updater-client --lib --locked
```

The tests use an IPv4 loopback server and fresh signing keys held only in memory.
Their HTTP transport exception exists only inside the test module. Production
configuration always enforces HTTPS. Inert signed fixture bytes are never executable
installers. Launch failure after the Windows pre-exit hook is simulated separately.

To run the real daemon pause/recovery test against an already built daemon (this
test uses an **idle daemon**, not an active analysis/worker workload):

```powershell
$env:IRIS_TEST_DAEMON = (Resolve-Path target/debug/iris-daemon.exe).Path
cargo test --manifest-path apps/shell/src-tauri/Cargo.toml --features updater-client --lib --locked real_daemon_update_pause_and_failed_install_recovery -- --ignored
```

The read-only release helper verifies the actual artifact, `.sig`, public key and
expected signed version. It neither installs the artifact nor accesses a private key:

```powershell
cargo run --manifest-path apps/shell/src-tauri/Cargo.toml --features updater-client --example verify_update_artifact --locked -- <installer.exe> <installer.exe.sig> <public-key-file> <version>
```

The `desktop` feature preserves Tauri's full default feature set; `updater-client`
allows these tools/tests to avoid Windows WebView/Common Controls activation.
Active-analysis shutdown is covered separately by an opt-in bounded workload test:

```powershell
pwsh -NoProfile -File tools/verify-update-active-workload.ps1
```

The script creates 36 distinct 2048×1536 synthetic JPGs in an owned temporary
directory and starts the real daemon with the default local models. It requires a
partial analysis result while actual analysis child processes remain alive, then
calls `pause_for_update` without invoking an installer. Windows process handles
prove that the observed workers exit; a read-only SQLite `quick_check`, restored
photo/results queries and a fresh session token verify recovery. Temporary photos
and database are removed after the run. Existing photographs are not read or edited.

The local run on 2026-10-07 passed with **12/36 photos completed and 12 workers
alive before pause**, a 50 ms daemon pause, all observed workers exited, 36 photo
records and 12 committed analysis results restored, SQLite `quick_check=ok`, and
no forced cleanup. Evidence: `artifacts/update-active-workload.json`. This is a
bounded synthetic workload result, not a guarantee for every workload or machine.

`tools/verify-nsis-lifecycle.ps1` separately exercised an actual isolated QA install,
installer metadata/marker replacement, and uninstall. Both stages verified 991
resources and installed host heartbeats; the external host-created SQLite database
and sentinel survived, and QA registration, shortcuts and installed files were
cleaned. The report is
`artifacts/nsis-qa-133a49d2955a457a836efd5362d4ffd1/lifecycle-report.json`.
Product, publisher, bundle and binary names were randomized. Both QA installer
versions use the same 0.1.0 application payload and never start the normal Tauri
entry point. This does not verify cross-application-version migration, updater
installation end to end, production-identity installation or a clean Windows VM.
No production signing key or public update endpoint is created by these tests.
