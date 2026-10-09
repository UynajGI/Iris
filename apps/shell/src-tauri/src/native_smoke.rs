//! Opt-in, fixed-script integration smoke test. Absent from normal desktop builds.
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{webview::PageLoadEvent, Manager};

pub struct Smoke {
    report: PathBuf,
    observations: Mutex<Vec<Value>>,
    injected: AtomicBool,
    finished: AtomicBool,
}

pub fn configure(
    builder: tauri::Builder<tauri::Wry>,
) -> anyhow::Result<tauri::Builder<tauri::Wry>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .is_none_or(|value| value != "--verify-native-title")
    {
        return Ok(builder);
    }
    let report = args.get(2).map(PathBuf::from).ok_or_else(|| {
        anyhow::anyhow!("--verify-native-title requires an absolute report filename")
    })?;
    anyhow::ensure!(report.is_absolute(), "smoke report must be absolute");
    anyhow::ensure!(!report.exists(), "smoke report already exists");
    Ok(builder
        .append_invoke_initialization_script(
            r#"
            window.__nativeTitleSmokeErrors = [];
            window.addEventListener('error', event => window.__nativeTitleSmokeErrors.push({
              message: event.message || 'resource error', file: event.filename || event.target?.src || ''
            }));
            window.addEventListener('unhandledrejection', event => window.__nativeTitleSmokeErrors.push({message: String(event.reason)}));
            window.addEventListener('iris:error', event => window.__nativeTitleSmokeErrors.push({message: String(event.detail)}));
            "#,
        )
        .manage(Smoke {
            report,
            observations: Mutex::new(Vec::new()),
            injected: AtomicBool::new(false),
            finished: AtomicBool::new(false),
        })
        .on_page_load(|webview, payload| {
            if webview.label() != "main" || payload.event() != PageLoadEvent::Finished {
                return;
            }
            let Some(state) = webview.try_state::<Smoke>() else {
                return;
            };
            if state.injected.swap(true, Ordering::SeqCst) {
                return;
            }
            if let Err(error) = webview.eval(include_str!("native_smoke.js")) {
                finish(webview.app_handle(), Some(error.to_string()));
            }
        }))
}

pub fn arm_timeout(app: tauri::AppHandle) {
    if app.try_state::<Smoke>().is_none() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(45));
        finish(
            &app,
            Some("native WebView smoke exceeded 45 seconds".into()),
        );
    });
}

#[tauri::command]
pub fn native_title_smoke_observe(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Smoke>,
    stage: String,
    document_title: String,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("smoke only accepts the main window".into());
    }
    let mut observations = state.observations.lock().map_err(|e| e.to_string())?;
    let expected = [
        ("english-before", "Iris"),
        ("chinese", "伊人"),
        ("english-after", "Iris"),
        ("invalid-rejected", "Iris"),
    ];
    let (expected_stage, expected_title) = expected
        .get(observations.len())
        .ok_or("unexpected extra smoke observation")?;
    let native_title = window.title().map_err(|e| e.to_string())?;
    let visible = window.is_visible().map_err(|e| e.to_string())?;
    if stage != *expected_stage
        || document_title != *expected_title
        || native_title != *expected_title
        || visible
    {
        return Err(format!(
            "smoke mismatch: stage={stage}, native={native_title}, document={document_title}, visible={visible}"
        ));
    }
    observations.push(json!({
        "stage":stage,"native_title":native_title,"document_title":document_title,"visible":visible
    }));
    Ok(())
}

fn finish(app: &tauri::AppHandle, error: Option<String>) {
    let Some(state) = app.try_state::<Smoke>() else {
        return;
    };
    if state.finished.swap(true, Ordering::SeqCst) {
        return;
    }
    let observations = state.observations.lock().unwrap();
    let error = error.or_else(|| {
        (observations.len() != 4).then(|| "missing native title observations".to_string())
    });
    let ok = error.is_none();
    let report = json!({"ok":ok,"error":error,"frontend_entry":"/src/desktop.js",
        "frontend_ready":ok,"updater_not_configured":ok,"invalid_locale_rejected":ok,"observations":*observations});
    let written = std::fs::write(&state.report, serde_json::to_vec_pretty(&report).unwrap());
    if let Err(error) = &written {
        eprintln!("write native smoke report failed: {error}");
    }
    app.exit(if ok && written.is_ok() { 0 } else { 1 });
}

#[tauri::command]
pub fn native_title_smoke_finish(app: tauri::AppHandle, error: Option<String>) {
    finish(&app, error);
}
