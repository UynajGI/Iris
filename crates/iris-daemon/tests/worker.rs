//! Protocol isolation tests use an intentionally fake child executable, not model inference.
use iris_core::vision::AnalysisSettings;
use iris_daemon::worker::AnalysisWorker;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};

fn executable() -> &'static Path {
    static FIXTURE: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    &FIXTURE
        .get_or_init(|| {
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("fake_worker.rs");
            std::fs::write(&source, FAKE_WORKER).unwrap();
            let executable = directory.path().join(if cfg!(windows) {
                "fake_worker.exe"
            } else {
                "fake_worker"
            });
            let output = std::process::Command::new("rustc")
                .args(["--edition", "2021"])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .expect("rustc needed for isolated fake-worker test fixture");
            assert!(
                output.status.success(),
                "fake worker compile failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            (directory, executable)
        })
        .1
}
fn worker(dir: &Path, timeout: Duration) -> AnalysisWorker {
    AnalysisWorker::with_executable(executable(), dir, timeout).unwrap()
}
fn starts(dir: &Path) -> u32 {
    std::fs::read_to_string(dir.join("starts"))
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn persistent_worker_reuses_process_and_roundtrips_numbered_requests() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = worker(dir.path(), Duration::from_secs(2));
    for _ in 0..3 {
        let value = child
            .analyze(Path::new("normal.jpg"), &AnalysisSettings::default())
            .unwrap();
        assert_eq!(value.version, "protocol-fixture");
    }
    assert_eq!(starts(dir.path()), 1);
}

#[test]
fn hard_deadline_kills_hung_child_and_next_request_starts_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = worker(dir.path(), Duration::from_millis(500));
    let start = Instant::now();
    let error = child
        .analyze(Path::new("hang.jpg"), &AnalysisSettings::default())
        .unwrap_err();
    assert!(error.to_string().contains("timed out"), "{error}");
    assert!(start.elapsed() < Duration::from_secs(4));
    child
        .analyze(Path::new("normal.jpg"), &AnalysisSettings::default())
        .unwrap();
    assert_eq!(starts(dir.path()), 2);
}

#[test]
fn killed_request_never_finishes_late_in_background() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = worker(dir.path(), Duration::from_millis(500));
    assert!(child
        .analyze(Path::new("late-marker.jpg"), &AnalysisSettings::default())
        .is_err());
    std::thread::sleep(Duration::from_millis(1800));
    assert!(
        !dir.path().join("late-result").exists(),
        "timed-out worker was detached instead of killed"
    );
}

#[test]
fn crash_malformed_response_and_wrong_request_id_are_recoverable() {
    for mode in ["crash.jpg", "malformed.jpg", "wrong-id.jpg"] {
        let dir = tempfile::tempdir().unwrap();
        let mut child = worker(dir.path(), Duration::from_secs(2));
        assert!(
            child
                .analyze(Path::new(mode), &AnalysisSettings::default())
                .is_err(),
            "{mode}"
        );
        child
            .analyze(Path::new("normal.jpg"), &AnalysisSettings::default())
            .unwrap();
        assert_eq!(starts(dir.path()), 2);
    }
}

#[test]
fn cancellation_terminates_active_native_work_before_its_deadline() {
    let dir = tempfile::tempdir().unwrap();
    let mut child = worker(dir.path(), Duration::from_secs(5));
    let cancel = Arc::new(AtomicBool::new(false));
    child.set_cancel(cancel.clone());
    let trigger = cancel.clone();
    let signal = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        trigger.store(true, Ordering::Relaxed);
    });
    let start = Instant::now();
    let error = child
        .analyze(Path::new("hang.jpg"), &AnalysisSettings::default())
        .unwrap_err();
    signal.join().unwrap();
    assert!(error.to_string().contains("cancelled"));
    assert!(start.elapsed() < Duration::from_secs(2));
    cancel.store(false, Ordering::Relaxed);
    child
        .analyze(Path::new("normal.jpg"), &AnalysisSettings::default())
        .unwrap();
    assert_eq!(starts(dir.path()), 2);
}

#[test]
fn startup_deadline_and_protocol_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    for mode in ["startup-hang", "bad-handshake", "startup-error"] {
        let models = dir.path().join(mode);
        std::fs::create_dir(&models).unwrap();
        let start = Instant::now();
        let result =
            AnalysisWorker::with_executable(executable(), &models, Duration::from_millis(500));
        assert!(result.is_err(), "{mode}");
        assert!(start.elapsed() < Duration::from_secs(4));
    }
}

#[test]
fn startup_cancel_after_child_spawn_keeps_typed_cancellation_cause() {
    let executable = executable();
    let directory = tempfile::tempdir().unwrap();
    let models = directory.path().join("startup-hang");
    std::fs::create_dir(&models).unwrap();
    let marker = models.join("starts");
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let signal = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(marker.exists(), "fake child never entered startup");
        trigger.store(true, Ordering::Relaxed);
    });
    let result = AnalysisWorker::with_executable_and_cancel(
        executable,
        &models,
        Duration::from_secs(5),
        cancel,
    );
    signal.join().unwrap();
    let error = result
        .err()
        .expect("startup cancellation should stop the child");
    assert!(
        error.is::<iris_daemon::worker::WorkerCancelled>(),
        "{error:#}"
    );
    assert_eq!(starts(&models), 1);
}

#[test]
fn real_worker_mode_does_not_create_database_or_start_http_when_models_missing() {
    let directory = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_iris-daemon"))
        .current_dir(directory.path())
        .args([
            "--internal-analysis-worker",
            "--model-dir",
            "missing-models",
        ])
        .output()
        .unwrap();
    let lines: Vec<_> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(lines.len(), 1);
    let ready: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(ready["protocol"], "iris-analysis-worker-v1");
    assert_eq!(ready["ready"], false);
    assert!(ready.get("base_url").is_none());
    assert!(!directory.path().join(".iris").exists());
}

#[test]
fn pre_cancelled_startup_returns_before_initializing_models() {
    let directory = tempfile::tempdir().unwrap();
    let cancel = Arc::new(AtomicBool::new(true));
    let result = AnalysisWorker::new_with_cancel(directory.path(), Duration::from_secs(10), cancel);
    assert!(result.err().unwrap().to_string().contains("cancelled"));
}

#[test]
fn invalid_worker_limits_exit_before_creating_database_or_starting_http() {
    for invalid in ["0", "17", "-1", "1.5", "invalid"] {
        let directory = tempfile::tempdir().unwrap();
        let data = directory.path().join("must-not-create");
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_iris-daemon"))
            .args(["--worker-limit", invalid, "--data-dir"])
            .arg(&data)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            output.stdout.is_empty(),
            "must not publish a server handshake"
        );
        assert!(!data.exists(), "invalid arguments must not create storage");
    }
}

#[test]
#[ignore = "requires provisioned real models and ONNX Runtime DLL"]
fn actual_models_run_in_persistent_daemon_child() {
    let directory = tempfile::tempdir().unwrap();
    let photo = directory.path().join("fixture.jpg");
    let hex="ffd8ffe000104a46494600010100000100010000ffdb004300100b0c0e0c0a100e0d0e1211101318281a181616183123251d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d71797064785c656763ffdb0043011112121815182f1a1a2f6342384263636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363ffc00011080002000203012200021101031101ffc4001500010100000000000000000000000000000005ffc40014100100000000000000000000000000000000ffc40014010100000000000000000000000000000002ffc40014110100000000000000000000000000000000ffda000c03010002110311003f008c0183ffd9";
    let bytes: Vec<_> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    std::fs::write(&photo, &bytes).unwrap();
    let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models");
    let mut child = AnalysisWorker::with_executable(
        Path::new(env!("CARGO_BIN_EXE_iris-daemon")),
        &models,
        Duration::from_secs(30),
    )
    .unwrap();
    for _ in 0..2 {
        let analysis = child.analyze(&photo, &AnalysisSettings::default()).unwrap();
        assert_eq!(analysis.version, iris_core::vision::ANALYSIS_VERSION);
        assert_eq!((analysis.width, analysis.height), (2, 2));
        assert!(analysis.composite_score.is_finite());
    }
    assert_eq!(std::fs::read(photo).unwrap(), bytes);
}

const FAKE_WORKER: &str = r###"
use std::{io::{BufRead,Write},time::Duration};
fn send(line:&str){println!("{line}");std::io::stdout().flush().unwrap();}
fn main(){
    let args:Vec<_>=std::env::args().collect();let model=std::path::PathBuf::from(&args[args.iter().position(|v|v=="--model-dir").unwrap()+1]);
    let counter=model.join("starts");let count=std::fs::read_to_string(&counter).ok().and_then(|v|v.parse::<u32>().ok()).unwrap_or(0)+1;std::fs::write(counter,count.to_string()).unwrap();
    let name=model.file_name().unwrap().to_string_lossy();
    if name=="startup-hang"{std::thread::sleep(Duration::from_secs(60));}
    if name=="bad-handshake"{send(r#"{"protocol":"wrong-v0","ready":true,"error":null}"#);return;}
    if name=="startup-error"{send(r#"{"protocol":"iris-analysis-worker-v1","ready":false,"error":"fixture startup error"}"#);return;}
    send(r#"{"protocol":"iris-analysis-worker-v1","ready":true,"error":null}"#);
    for line in std::io::stdin().lock().lines(){let line=line.unwrap();let rest=line.split("\"id\":").nth(1).unwrap();let id=rest.chars().take_while(|c|c.is_ascii_digit()).collect::<String>().parse::<u64>().unwrap();
        if line.contains("hang.jpg"){std::thread::sleep(Duration::from_secs(60));}
        if line.contains("late-marker.jpg"){std::thread::sleep(Duration::from_secs(2));std::fs::write(model.join("late-result"),b"finished").unwrap();}
        if line.contains("crash.jpg"){std::process::exit(7);}
        if line.contains("malformed.jpg"){send("not json");continue;}
        let id=if line.contains("wrong-id.jpg"){id+1}else{id};
        send(&format!(r#"{{"id":{id},"analysis":{{"width":1,"height":1,"original_width":1,"original_height":1,"orientation":1,"faces":[],"sharpness_lap":0.0,"sharpness_fft":0.0,"niqe":null,"exposure":{{"mean":128.0,"shadow_clip":0.0,"highlight_clip":0.0,"verdict":"normal"}},"composite_score":0.0,"verdict":"review","phash":"0","structure":[],"warnings":[],"version":"protocol-fixture"}},"error":null,"fatal":false}}"#));
    }
}
"###;
