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
    window_checks: Mutex<Vec<String>>,
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
            window_checks: Mutex::new(Vec::new()),
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
        || !visible
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
    let checks = state.window_checks.lock().unwrap();
    let report = json!({"ok":ok,"error":error,"frontend_entry":"/app.js",
        "frontend_ready":ok,"updater_not_configured":ok,"invalid_locale_rejected":ok,
        "observations":*observations,"window_checks":*checks});
    let written = std::fs::write(&state.report, serde_json::to_vec_pretty(&report).unwrap());
    if let Err(error) = &written {
        eprintln!("write native smoke report failed: {error}");
    }
    if ok && written.is_ok() {
        super::window_lifecycle::request_exit(app);
    } else {
        app.exit(1);
    }
}

#[tauri::command]
pub fn native_title_smoke_finish(app: tauri::AppHandle, error: Option<String>) {
    finish(&app, error);
}

/// Fixed, opt-in actions for testing this application's own native lifecycle.
#[tauri::command]
pub fn native_window_smoke(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Smoke>,
    action: String,
) -> Result<Value, String> {
    if window.label() != "main" {
        return Err("smoke only accepts main".into());
    }
    perform_window_action(&window, &state, &action)?;
    let result = json!({"visible":window.is_visible().map_err(|e| e.to_string())?,
        "minimized":window.is_minimized().map_err(|e| e.to_string())?,
        "maximized":window.is_maximized().map_err(|e| e.to_string())?,
        "tray":window.app_handle().tray_by_id("iris").is_some(),
        "caption":caption_hit_tests(&window)?});
    if action != "status" {
        state.window_checks.lock().unwrap().push(action);
    }
    Ok(result)
}

fn perform_window_action(
    window: &tauri::WebviewWindow,
    state: &Smoke,
    action: &str,
) -> Result<(), String> {
    match action {
        "show" => super::window_lifecycle::show_main(window.app_handle()),
        "minimize" => window.minimize().map_err(|e| e.to_string())?,
        "maximize" => window.maximize().map_err(|e| e.to_string())?,
        "restore" => restore_window(window)?,
        "close" => window.close().map_err(|e| e.to_string())?,
        "tray-exit" => super::window_lifecycle::request_exit(window.app_handle()),
        "capture" | "dialog" => mark_capture(state, action)?,
        "status" => {}
        _ => return Err("unknown smoke action".into()),
    }
    Ok(())
}

fn restore_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    window.unmaximize().map_err(|e| e.to_string())?;
    window.unminimize().map_err(|e| e.to_string())
}

fn mark_capture(state: &Smoke, action: &str) -> Result<(), String> {
    std::fs::write(
        state.report.with_extension(format!("{action}-ready")),
        b"ready",
    )
    .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn caption_hit_tests(window: &tauri::WebviewWindow) -> Result<Vec<isize>, String> {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CAPTION_BUTTON_BOUNDS},
        UI::{
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{GetWindowRect, SendMessageW, WM_NCHITTEST},
        },
    };
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
    // SAFETY: fixed system queries against the live test-owned window on its UI thread.
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        let mut buttons: RECT = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect);
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_BUTTON_BOUNDS as u32,
            &mut buttons as *mut _ as _,
            std::mem::size_of::<RECT>() as u32,
        );
        if hr < 0 {
            return Err(format!("caption bounds query failed: {hr}"));
        }
        let scale = GetDpiForWindow(hwnd) as i32;
        let y = rect.top + (buttons.top + buttons.bottom) / 2;
        Ok([115, 69, 23]
            .map(|offset| {
                let x = rect.left + buttons.right - offset * scale / 96;
                let position = ((y as u16 as u32) << 16) | (x as u16 as u32);
                SendMessageW(hwnd, WM_NCHITTEST, 0, position as isize)
            })
            .to_vec())
    }
}

#[cfg(not(windows))]
fn caption_hit_tests(_window: &tauri::WebviewWindow) -> Result<Vec<isize>, String> {
    Ok(Vec::new())
}
