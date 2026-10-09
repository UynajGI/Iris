//! Process boundary only: no domain logic or source-photo access in the shell.
#[cfg(feature = "updater-client")]
pub mod updater;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

/// Fixed runtime window names; never accept arbitrary title text from the webview.
#[derive(Clone, Copy, Deserialize)]
pub enum AppLocale {
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}
impl AppLocale {
    pub fn product_name(self) -> &'static str {
        match self {
            Self::Chinese => "伊人",
            Self::English => "IrisVision",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Session {
    pub base_url: String,
    pub token: String,
    pub version: String,
}
impl Session {
    pub fn validate(&self) -> Result<SocketAddr> {
        let address: SocketAddr = self
            .base_url
            .strip_prefix("http://")
            .context("daemon must use HTTP loopback")?
            .parse()?;
        if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            || address.port() == 0
        {
            bail!("daemon must bind IPv4 loopback and a nonzero port");
        }
        if self.token.is_empty()
            || !self
                .token
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            bail!("invalid daemon session token");
        }
        if self.version.split('.').next() != env!("CARGO_PKG_VERSION").split('.').next() {
            bail!("daemon protocol version is incompatible");
        }
        Ok(address)
    }
    pub fn heartbeat(&self) -> Result<()> {
        let address = self.validate()?;
        let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
        socket.set_read_timeout(Some(Duration::from_secs(2)))?;
        socket.set_write_timeout(Some(Duration::from_secs(2)))?;
        write!(socket, "GET /api/v1/bootstrap HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n", self.token)?;
        let mut status = String::new();
        BufReader::new(socket).take(1024).read_line(&mut status)?;
        if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
            bail!("daemon heartbeat failed");
        }
        Ok(())
    }
}

pub struct DaemonHost {
    executable: PathBuf,
    data_dir: PathBuf,
    model_dir: Option<PathBuf>,
    child: Option<Child>,
    session: Option<Session>,
    failures: u8,
    update_paused: bool,
}
impl DaemonHost {
    pub fn new(executable: impl AsRef<Path>, data_dir: impl AsRef<Path>) -> Self {
        Self {
            executable: executable.as_ref().into(),
            data_dir: data_dir.as_ref().into(),
            model_dir: None,
            child: None,
            session: None,
            failures: 0,
            update_paused: false,
        }
    }
    pub fn with_model_dir(mut self, directory: impl AsRef<Path>) -> Self {
        self.model_dir = Some(directory.as_ref().into());
        self
    }
    pub fn model_dir(&self) -> Result<PathBuf> {
        let executable = self
            .executable
            .canonicalize()
            .context("resolve daemon executable")?;
        let sibling = executable
            .parent()
            .context("daemon executable has no parent")?;
        let directory = self
            .model_dir
            .clone()
            .or_else(|| std::env::var_os("IRIS_MODEL_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                let direct = sibling.join("models");
                if direct.is_dir() {
                    direct
                } else {
                    sibling.join("resources").join("models")
                }
            });
        let directory = directory.canonicalize().context(
            "resolve installed models; set IRIS_MODEL_DIR to an absolute model directory",
        )?;
        if !directory.is_dir() {
            bail!("model path is not a directory");
        }
        Ok(directory)
    }
    pub fn start(&mut self) -> Result<Session> {
        anyhow::ensure!(!self.update_paused, "daemon is paused for installation");
        self.stop();
        let executable = self
            .executable
            .canonicalize()
            .context("resolve daemon executable")?;
        let models = self.model_dir()?;
        std::fs::create_dir_all(&self.data_dir)?;
        let data_dir = self.data_dir.canonicalize()?;
        let mut command = Command::new(executable);
        command
            .args(["--data-dir"])
            .arg(data_dir)
            .arg("--model-dir")
            .arg(models)
            .args(["--port", "0"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().context("start iris-daemon")?;
        let stdout = child
            .stdout
            .take()
            .context("daemon bootstrap pipe missing")?;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            let result = (&mut reader)
                .take(16384)
                .read_line(&mut line)
                .map(|_| line)
                .map_err(|e| e.to_string());
            let _ = sender.send(result);
            // Drain subsequent output without reflecting credentials to logs.
            let _ = std::io::copy(&mut reader, &mut std::io::sink());
        });
        self.child = Some(child);
        let result = (|| -> Result<Session> {
            let line = receiver
                .recv_timeout(Duration::from_secs(30))
                .context("daemon bootstrap timed out")?
                .map_err(anyhow::Error::msg)?;
            let session: Session =
                serde_json::from_str(&line).context("invalid daemon bootstrap response")?;
            session.validate()?;
            session.heartbeat()?;
            Ok(session)
        })();
        match result {
            Ok(session) => {
                self.session = Some(session.clone());
                self.failures = 0;
                Ok(session)
            }
            Err(error) => {
                self.stop();
                Err(error)
            }
        }
    }
    pub fn session(&self) -> Result<Session> {
        anyhow::ensure!(!self.update_paused, "daemon is paused for installation");
        self.session.clone().context("daemon is not running")
    }
    /// Returns Some(new session) after restart so clients rotate their token.
    pub fn supervise(&mut self) -> Result<Option<Session>> {
        if self.update_paused {
            return Ok(None);
        }
        let exited = match self.child.as_mut() {
            Some(child) => child.try_wait()?.is_some(),
            None => true,
        };
        if exited {
            return self.start().map(Some);
        }
        if self
            .session
            .as_ref()
            .context("missing session")?
            .heartbeat()
            .is_err()
        {
            self.failures += 1;
            if self.failures >= 3 {
                return self.start().map(Some);
            }
        } else {
            self.failures = 0;
        }
        Ok(None)
    }
    /// Installation requires a confirmed graceful exit. Never install after a
    /// timeout by merely killing the daemon: analysis children may still hold files.
    pub fn pause_for_update(&mut self) -> Result<()> {
        self.pause_for_update_timeout(Duration::from_secs(15))
    }
    fn pause_for_update_timeout(&mut self, timeout: Duration) -> Result<()> {
        self.update_paused = true;
        self.session = None;
        if let Some(child) = self.child.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                // An already exited child can have a broken pipe; try_wait below
                // remains the authority on whether the executable is released.
                let _ = stdin.write_all(b"shutdown\n");
            }
            let deadline = Instant::now() + timeout;
            loop {
                if let Some(status) = child.try_wait()? {
                    self.child = None;
                    anyhow::ensure!(
                        status.success(),
                        "daemon exited abnormally; installation cancelled"
                    );
                    return Ok(());
                }
                anyhow::ensure!(
                    Instant::now() < deadline,
                    "daemon did not exit gracefully; installation cancelled"
                );
                std::thread::sleep(Duration::from_millis(25));
            }
        }
        Ok(())
    }
    pub fn resume_after_update_failure(&mut self) -> Result<Session> {
        self.update_paused = false;
        self.start()
    }
    pub fn stop(&mut self) {
        self.session = None;
        if let Some(mut child) = self.child.take() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(b"shutdown\n");
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Err(_) => break,
                    Ok(None) => {}
                }
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
impl Drop for DaemonHost {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(all(test, windows))]
#[path = "updater_workload_test.rs"]
mod updater_workload_test;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_locale_maps_only_supported_locales_to_product_names() {
        for (locale, title) in [("zh-CN", "伊人"), ("en", "IrisVision")] {
            let parsed: AppLocale = serde_json::from_value(serde_json::json!(locale)).unwrap();
            assert_eq!(parsed.product_name(), title);
        }
    }
    #[test]
    fn app_locale_rejects_arbitrary_titles_and_unrecognized_locales() {
        for value in [
            serde_json::json!("arbitrary title"),
            serde_json::json!("zh-TW"),
            serde_json::json!("en-US"),
            serde_json::json!({"title": "another app"}),
            serde_json::Value::Null,
        ] {
            assert!(serde_json::from_value::<AppLocale>(value).is_err());
        }
    }
    #[test]
    fn validates_boundary_and_never_allows_header_injection() {
        let mut session = Session {
            base_url: "http://127.0.0.1:54321".into(),
            token: "safe-token_123".into(),
            version: "0.1.0".into(),
        };
        assert!(session.validate().is_ok());
        session.token = "bad\r\nHeader: value".into();
        assert!(session.validate().is_err());
        session.token = "safe".into();
        session.base_url = "http://192.168.1.2:54321".into();
        assert!(session.validate().is_err());
        session.base_url = "http://127.0.0.1:0".into();
        assert!(session.validate().is_err());
        session.base_url = "http://127.0.0.1:54321".into();
        session.version = "99.0.0".into();
        assert!(session.validate().is_err());
    }
    #[test]
    fn startup_failure_clears_session() {
        let mut host = DaemonHost::new("missing-iris-daemon-executable", ".");
        assert!(host.start().is_err());
        assert!(host.session().is_err());
    }
    #[test]
    fn installation_pause_blocks_session_start_and_supervisor_until_recovery() {
        let mut host = DaemonHost::new("missing-iris-daemon-executable", ".");
        host.pause_for_update().unwrap();
        assert!(host.update_paused);
        assert!(host.session().is_err());
        assert!(host.start().is_err());
        // The missing executable would fail if supervision attempted a restart.
        for _ in 0..3 {
            assert!(host.supervise().unwrap().is_none());
        }
        assert!(host.resume_after_update_failure().is_err());
        assert!(!host.update_paused);
        assert!(host.supervise().is_err());
    }
    #[cfg(windows)]
    #[test]
    fn update_shutdown_timeout_does_not_kill_or_install_over_live_process() {
        use std::os::windows::process::CommandExt;
        let child = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 30",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let mut host = DaemonHost::new("missing-iris-daemon-executable", ".");
        host.child = Some(child);
        assert!(host.pause_for_update_timeout(Duration::ZERO).is_err());
        assert!(host.update_paused);
        assert!(host.child.as_mut().unwrap().try_wait().unwrap().is_none());
        assert!(host.supervise().unwrap().is_none());
        host.child.as_mut().unwrap().kill().unwrap();
        host.child.as_mut().unwrap().wait().unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn abnormal_daemon_exit_cancels_installation_and_keeps_supervision_paused() {
        use std::os::windows::process::CommandExt;
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "exit 17"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        assert!(!child.wait().unwrap().success());
        let mut host = DaemonHost::new("missing-iris-daemon-executable", ".");
        host.child = Some(child);
        assert!(host.pause_for_update().is_err());
        assert!(host.update_paused);
        assert!(host.supervise().unwrap().is_none());
        assert!(host.child.is_none());
    }
    #[test]
    #[ignore = "requires IRIS_TEST_DAEMON pointing to a built iris-daemon executable"]
    fn real_daemon_update_pause_and_failed_install_recovery() {
        let executable = std::env::var_os("IRIS_TEST_DAEMON").expect("IRIS_TEST_DAEMON required");
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = temp.join(format!(
            "iris-update-host-test-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut host = DaemonHost::new(executable, &directory)
            .with_model_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models"));
        let first = host.start().unwrap();
        first.heartbeat().unwrap();
        host.pause_for_update().unwrap();
        assert!(host.child.is_none());
        assert!(host.session().is_err());
        assert!(first.heartbeat().is_err());
        for _ in 0..3 {
            assert!(host.supervise().unwrap().is_none());
        }
        let second = host.resume_after_update_failure().unwrap();
        assert_ne!(first.token, second.token);
        second.heartbeat().unwrap();
        assert!(host.supervise().unwrap().is_none());
        host.stop();
        assert!(second.heartbeat().is_err());
        let directory = directory.canonicalize().unwrap();
        assert!(directory.starts_with(&temp) && directory != temp);
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    #[ignore = "requires IRIS_TEST_DAEMON pointing to a built iris-daemon executable"]
    fn real_daemon_restarts_with_fresh_token_and_exits_gracefully() {
        let executable = std::env::var_os("IRIS_TEST_DAEMON").expect("IRIS_TEST_DAEMON required");
        let temp = std::env::temp_dir().canonicalize().unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = temp.join(format!("iris-host-test-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let mut host = DaemonHost::new(executable, &directory)
            .with_model_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models"));
        assert!(host.model_dir().unwrap().is_absolute());
        let first = host.start().unwrap();
        first.heartbeat().unwrap();
        host.child.as_mut().unwrap().kill().unwrap();
        host.child.as_mut().unwrap().wait().unwrap();
        let second = host.supervise().unwrap().expect("crashed daemon restarted");
        assert_ne!(first.token, second.token);
        second.heartbeat().unwrap();
        let start = Instant::now();
        host.stop();
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "graceful exit should precede kill fallback"
        );
        assert!(second.heartbeat().is_err());
        let directory = directory.canonicalize().unwrap();
        assert!(directory.starts_with(&temp) && directory != temp);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
