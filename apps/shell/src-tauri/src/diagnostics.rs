//! Bounded, local event journal. Never accepts free-form text, paths or credentials.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 256 * 1024;
const ARCHIVES: usize = 3;
static LOG: OnceLock<Journal> = OnceLock::new();
static WRITE_FAILED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    AppStarted,
    AppExited,
    AppPanicked,
    StartupFailed,
    DaemonStarting,
    DaemonReady,
    DaemonBootstrapFailed,
    DaemonUnavailable,
    DaemonRestarted,
    DaemonStderr,
    FrontendFailed,
    RenderFailed,
    ConnectionLost,
    TaskStarted,
    TaskCompleted,
    TaskFailed,
    TaskCancelled,
    UpdateFailed,
    DiagnosticExported,
}

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub time_unix_ms: u64,
    pub event: Event,
}

pub struct Journal {
    directory: PathBuf,
    last: Mutex<Option<(Event, Instant)>>,
}

impl Journal {
    pub fn new(directory: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(directory)?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("iris.log"))?;
        Ok(Self {
            directory: directory.into(),
            last: Mutex::new(None),
        })
    }

    pub fn record(&self, event: Event) -> std::io::Result<()> {
        let mut last = self
            .last
            .lock()
            .map_err(|_| std::io::Error::other("journal unavailable"))?;
        if last
            .as_ref()
            .is_some_and(|(old, time)| *old == event && time.elapsed() < Duration::from_secs(2))
        {
            return Ok(());
        }
        let path = self.directory.join("iris.log");
        if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) >= MAX_BYTES {
            self.rotate()?;
        }
        let entry = Entry {
            time_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            event,
        };
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        serde_json::to_writer(&mut file, &entry)?;
        file.write_all(b"\n")?;
        *last = Some((event, Instant::now()));
        Ok(())
    }

    fn rotate(&self) -> std::io::Result<()> {
        let oldest = self.directory.join(format!("iris.{ARCHIVES}.log"));
        if oldest.exists() {
            fs::remove_file(oldest)?;
        }
        for index in (1..ARCHIVES).rev() {
            let source = self.directory.join(format!("iris.{index}.log"));
            if source.exists() {
                fs::rename(
                    source,
                    self.directory.join(format!("iris.{}.log", index + 1)),
                )?;
            }
        }
        fs::rename(
            self.directory.join("iris.log"),
            self.directory.join("iris.1.log"),
        )
    }

    pub fn snapshot(&self) -> std::io::Result<Vec<Entry>> {
        let _guard = self
            .last
            .lock()
            .map_err(|_| std::io::Error::other("journal unavailable"))?;
        let names = (1..=ARCHIVES)
            .rev()
            .map(|i| format!("iris.{i}.log"))
            .chain(std::iter::once("iris.log".into()));
        let mut entries = Vec::new();
        for name in names {
            let path = self.directory.join(name);
            if !path.exists() {
                continue;
            }
            let mut text = String::new();
            File::open(path)?
                .take(MAX_BYTES + 1024)
                .read_to_string(&mut text)?;
            // Decode only our typed schema: no raw log bytes enter a support report.
            entries.extend(
                text.lines()
                    .filter_map(|line| serde_json::from_str::<Entry>(line).ok()),
            );
        }
        Ok(entries)
    }
}

pub fn initialize(data_dir: &Path) -> bool {
    if LOG.get().is_some() {
        return true;
    }
    Journal::new(&data_dir.join("logs")).is_ok_and(|journal| LOG.set(journal).is_ok())
}
pub fn available() -> bool {
    LOG.get().is_some() && !WRITE_FAILED.load(Ordering::Relaxed)
}
pub fn record(event: Event) {
    if let Some(journal) = LOG.get() {
        WRITE_FAILED.store(journal.record(event).is_err(), Ordering::Relaxed);
    }
}
pub fn snapshot() -> std::io::Result<Vec<Entry>> {
    LOG.get()
        .ok_or_else(|| std::io::Error::other("日志目录不可写，请检查磁盘空间与权限"))?
        .snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotates_bounds_and_exports_only_typed_events() {
        let directory = std::env::temp_dir().join(format!(
            "iris-journal-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = Journal::new(&directory).unwrap();
        for _ in 0..6 {
            fs::write(directory.join("iris.log"), vec![b'x'; MAX_BYTES as usize]).unwrap();
            journal.record(Event::AppStarted).unwrap();
            journal.record(Event::AppExited).unwrap();
        }
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 4);
        assert_eq!(journal.snapshot().unwrap().len(), 2);
        let mut file = OpenOptions::new()
            .append(true)
            .open(directory.join("iris.log"))
            .unwrap();
        writeln!(file, "{{\"time_unix_ms\":1,\"event\":\"frontend_failed\",\"token\":\"secret\",\"path\":\"private.jpg\"}}").unwrap();
        let report = serde_json::to_string(&journal.snapshot().unwrap()).unwrap();
        assert!(!report.contains("secret") && !report.contains("private.jpg"));
        assert!(serde_json::from_str::<Event>("\"arbitrary user message\"").is_err());
        drop(file);
        fs::remove_dir_all(directory).unwrap();
    }
}
