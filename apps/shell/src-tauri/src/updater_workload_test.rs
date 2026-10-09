//! Opt-in real workload test. Owns only a temporary database and synthetic photos.
use super::*;
use serde_json::{json, Value};
use std::{ffi::c_void, os::windows::process::CommandExt};

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn WaitForSingleObject(handle: *mut c_void, millis: u32) -> u32;
    fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
struct ObservedProcess {
    pid: u32,
    handle: *mut c_void,
}
impl ObservedProcess {
    fn open(pid: u32) -> Result<Self> {
        // Retain the kernel process object, so a recycled PID cannot fake a result.
        let handle = unsafe { OpenProcess(0x00100000 | 0x0001, 0, pid) };
        anyhow::ensure!(!handle.is_null(), "cannot observe owned process {pid}");
        Ok(Self { pid, handle })
    }
    fn exited(&self) -> bool {
        unsafe { WaitForSingleObject(self.handle, 0) == 0 }
    }
    fn wait(&self, millis: u32) -> bool {
        unsafe { WaitForSingleObject(self.handle, millis) == 0 }
    }
}
impl Drop for ObservedProcess {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
fn worker_pids(parent: u32) -> Result<Vec<u32>> {
    let script = format!("@(Get-CimInstance Win32_Process -Filter 'ParentProcessId = {parent}' | Where-Object {{ $_.CommandLine -like '*--internal-analysis-worker*' }} | ForEach-Object {{ [int]$_.ProcessId }}) | ConvertTo-Json -Compress");
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "cannot enumerate analysis children"
    );
    let text = String::from_utf8(output.stdout)?;
    if text.trim().is_empty() {
        return Ok(vec![]);
    }
    let value: Value = serde_json::from_str(&text)?;
    Ok(match value {
        Value::Array(values) => values
            .iter()
            .filter_map(|v| v.as_u64().map(|v| v as u32))
            .collect(),
        Value::Number(value) => vec![value.as_u64().context("invalid PID")? as u32],
        _ => vec![],
    })
}
fn request(session: &Session, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
    let address = session.validate()?;
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(3)))?;
    let body = body
        .map(|value| serde_json::to_vec(&value))
        .transpose()?
        .unwrap_or_default();
    write!(socket, "{method} {path} HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", session.token, body.len())?;
    socket.write_all(&body)?;
    let mut response = Vec::new();
    socket.read_to_end(&mut response)?;
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .context("missing HTTP headers")?;
    let headers = std::str::from_utf8(&response[..split])?;
    let status = headers
        .split_whitespace()
        .nth(1)
        .context("missing HTTP status")?;
    anyhow::ensure!(
        status.starts_with('2'),
        "test HTTP request failed with {status}"
    );
    // Axum's JSON endpoints include Content-Length; do not silently misparse a
    // streaming response as a success if that contract ever changes.
    anyhow::ensure!(
        !headers
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked"),
        "unexpected chunked test response"
    );
    Ok(serde_json::from_slice(&response[split + 4..])?)
}

#[test]
#[ignore = "requires IRIS_TEST_DAEMON, IRIS_TEST_PHOTOS, IRIS_TEST_UPDATE_REPORT; run tools/verify-update-active-workload.ps1"]
fn real_active_analysis_workers_exit_before_update_and_database_recovers() {
    let executable = std::env::var_os("IRIS_TEST_DAEMON").expect("IRIS_TEST_DAEMON required");
    let photos =
        PathBuf::from(std::env::var_os("IRIS_TEST_PHOTOS").expect("IRIS_TEST_PHOTOS required"));
    let report_path = PathBuf::from(
        std::env::var_os("IRIS_TEST_UPDATE_REPORT").expect("IRIS_TEST_UPDATE_REPORT required"),
    );
    let temp = std::env::temp_dir().canonicalize().unwrap();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = temp.join(format!(
        "iris-update-active-db-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut host = DaemonHost::new(executable, &directory)
        .with_model_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models"));
    let mut processes = Vec::<ObservedProcess>::new();
    let mut report = json!({"ok":false, "fixture":"temporary synthetic JPGs", "installer_invoked":false, "forced_cleanup":false});
    let result = (|| -> Result<()> {
        let first = host.start()?;
        let daemon_pid = host.child.as_ref().context("daemon child absent")?.id();
        let daemon_handle = ObservedProcess::open(daemon_pid)?;
        let project = request(
            &first,
            "POST",
            "/api/v1/projects",
            Some(json!({"root":photos})),
        )?;
        let id = project["id"].as_i64().context("missing project id")?;
        let progress_path = format!("/api/v1/projects/{id}/progress");
        request(&first, "POST", &format!("/api/v1/projects/{id}/scan"), None)?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let count = loop {
            let progress = request(&first, "GET", &progress_path, None)?;
            if progress["state"] == "completed" {
                anyhow::ensure!(
                    progress["errors"].as_array().is_some_and(|v| v.is_empty()),
                    "scan errors"
                );
                break progress["completed"]
                    .as_u64()
                    .context("missing scan count")?;
            }
            anyhow::ensure!(progress["state"] == "running", "scan did not complete");
            anyhow::ensure!(Instant::now() < deadline, "scan deadline exceeded");
            std::thread::sleep(Duration::from_millis(100));
        };
        anyhow::ensure!(
            (24..=64).contains(&count),
            "expected 24..64 bounded test photos, got {count}"
        );
        request(
            &first,
            "POST",
            &format!("/api/v1/projects/{id}/analyze"),
            None,
        )?;
        let deadline = Instant::now() + Duration::from_secs(90);
        let active = loop {
            for pid in worker_pids(daemon_pid)? {
                if !processes.iter().any(|process| process.pid == pid) {
                    if let Ok(process) = ObservedProcess::open(pid) {
                        processes.push(process);
                    }
                }
            }
            let progress = request(&first, "GET", &progress_path, None)?;
            let completed = progress["completed"].as_u64().unwrap_or(0);
            let total = progress["total"].as_u64().unwrap_or(0);
            let alive = processes.iter().filter(|process| !process.exited()).count();
            report["last_observed_progress"] = progress.clone();
            report["last_observed_live_workers"] = json!(alive);
            if progress["kind"] == "analysis"
                && progress["state"] == "running"
                && completed > 0
                && completed < total
                && alive > 0
            {
                anyhow::ensure!(
                    progress["errors"].as_array().is_some_and(|v| v.is_empty()),
                    "analysis errors before pause"
                );
                report["active_workers"] = json!(alive);
                break progress;
            }
            anyhow::ensure!(progress["state"] == "running", "analysis completed before a partial-work/active-worker observation; scenario not covered");
            anyhow::ensure!(
                Instant::now() < deadline,
                "active worker observation deadline exceeded; scenario not covered"
            );
            std::thread::sleep(Duration::from_millis(100));
        };
        report["photo_count"] = json!(count);
        report["progress_before_pause"] = active;
        report["worker_pids"] = json!(processes
            .iter()
            .map(|process| process.pid)
            .collect::<Vec<_>>());
        let started = Instant::now();
        host.pause_for_update()?;
        report["pause_duration_ms"] = json!(started.elapsed().as_millis());
        anyhow::ensure!(
            daemon_handle.wait(1000),
            "daemon process remains alive after pause"
        );
        for worker in &processes {
            anyhow::ensure!(
                worker.wait(5000),
                "analysis worker {} survived daemon pause",
                worker.pid
            );
        }
        anyhow::ensure!(
            worker_pids(daemon_pid)?.is_empty(),
            "new analysis children survived pause"
        );
        anyhow::ensure!(host.session().is_err(), "paused host exposed a session");
        anyhow::ensure!(
            host.supervise()?.is_none(),
            "paused supervisor restarted daemon"
        );
        anyhow::ensure!(
            first.heartbeat().is_err(),
            "old daemon still accepts requests"
        );
        report["daemon_exited"] = json!(true);
        report["workers_exited"] = json!(true);
        let sqlite = Command::new("python")
            .args(["-c", "import sqlite3,sys,pathlib; c=sqlite3.connect(pathlib.Path(sys.argv[1]).as_uri()+'?mode=ro',uri=True); result=c.execute('PRAGMA quick_check').fetchall(); assert result==[('ok',)], result; print('ok'); c.close()"])
            .arg(directory.join("library.sqlite3")).creation_flags(0x08000000).output()?;
        anyhow::ensure!(
            sqlite.status.success() && String::from_utf8_lossy(&sqlite.stdout).trim() == "ok",
            "SQLite read-only quick_check failed"
        );
        report["database_reopened_readonly"] = json!(true);
        report["sqlite_quick_check"] = json!("ok");
        let second = host.resume_after_update_failure()?;
        anyhow::ensure!(
            first.token != second.token,
            "recovery did not rotate session token"
        );
        second.heartbeat()?;
        let persisted = request(
            &second,
            "GET",
            &format!("/api/v1/projects/{id}/photos"),
            None,
        )?;
        let persisted = persisted
            .as_array()
            .context("missing restored photo records")?;
        anyhow::ensure!(
            persisted.len() == count as usize,
            "photo records changed during recovery"
        );
        let analyzed = persisted
            .iter()
            .filter(|photo| !photo["analysis"].is_null())
            .count();
        anyhow::ensure!(analyzed > 0, "committed analysis results were lost");
        report["restored_photo_count"] = json!(persisted.len());
        report["persisted_analysis_count"] = json!(analyzed);
        report["fresh_session_token"] = json!(true);
        report["ok"] = json!(true);
        Ok(())
    })();
    host.stop();
    // Even a failed assertion must not leave owned test workers running.
    for process in &processes {
        if !process.exited() {
            report["forced_cleanup"] = json!(true);
            unsafe {
                TerminateProcess(process.handle, 255);
            }
            process.wait(5000);
        }
    }
    if let Err(error) = &result {
        report["error"] = json!(error.to_string());
    }
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(report_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    let directory = directory.canonicalize().unwrap();
    assert!(directory.starts_with(&temp) && directory != temp);
    std::fs::remove_dir_all(directory).unwrap();
    result.unwrap();
}
