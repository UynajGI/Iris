#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use iris_shell::{updater, AppLocale, DaemonHost, Session};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

#[cfg(feature = "native-smoke")]
mod native_smoke;

fn daemon_executable() -> std::io::Result<std::path::PathBuf> {
    Ok(std::env::var_os("IRIS_DAEMON_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_exe()?.with_file_name(if cfg!(windows) {
            "iris-daemon.exe"
        } else {
            "iris-daemon"
        })))
}

// The portable verifier exercises the same host without creating a visual window.
fn verify_host(report: &std::path::Path) -> anyhow::Result<()> {
    let data_dir =
        std::env::var_os("IRIS_DATA_DIR").context("IRIS_DATA_DIR required for verification")?;
    let mut host = DaemonHost::new(daemon_executable()?, data_dir);
    let models = host.model_dir()?;
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(models.join("manifest.json"))?)?;
    let mut files = vec![
        "onnxruntime.dll".to_string(),
        "niqe_params.json".to_string(),
    ];
    for model in manifest["models"]
        .as_array()
        .context("model manifest missing models")?
    {
        files.push(
            model["file"]
                .as_str()
                .context("model manifest missing filename")?
                .into(),
        );
    }
    for file in &files {
        let path = models.join(file).canonicalize()?;
        anyhow::ensure!(
            path.starts_with(&models) && path.is_file(),
            "invalid packaged model path"
        );
    }
    let session = host.start()?;
    session.heartbeat()?;
    host.stop();
    std::fs::write(
        report,
        serde_json::to_vec_pretty(&serde_json::json!({
            "ok": true, "heartbeat": true, "version": session.version,
            "model_dir": models, "files": files, "credentials_logged": false
        }))?,
    )?;
    Ok(())
}
use anyhow::Context;

struct Host(Arc<Mutex<DaemonHost>>);
#[derive(Default)]
struct UnsavedChanges(AtomicBool);
#[tauri::command]
fn set_unsaved_changes(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, UnsavedChanges>,
    dirty: bool,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the main window can change its close guard".into());
    }
    state.0.store(dirty, Ordering::SeqCst);
    Ok(())
}
#[tauri::command]
fn confirm_close(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the main window can confirm closing".into());
    }
    window.destroy().map_err(|error| error.to_string())
}
#[tauri::command]
fn frontend_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the main window can report readiness".into());
    }
    window.show().map_err(|error| error.to_string())
}
#[tauri::command]
fn set_app_locale(window: tauri::WebviewWindow, locale: AppLocale) -> Result<(), String> {
    if window.label() != "main" {
        return Err("App locale is only available to the main window".into());
    }
    window
        .set_title(locale.product_name())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn select_project_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    // Native dialogs may wait indefinitely for the user; never block the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|selection| {
                let path = selection.into_path().map_err(|error| error.to_string())?;
                if !path.is_dir() {
                    return Err("Selected folder is no longer available".to_string());
                }
                path.into_os_string()
                    .into_string()
                    .map_err(|_| "Selected folder path cannot be represented as UTF-8".to_string())
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn select_export_csv(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("CSV", &["csv"])
            .blocking_save_file()
            .map(|selection| {
                selection
                    .into_path()
                    .map_err(|error| error.to_string())?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "Selected file path cannot be represented as UTF-8".to_string())
            })
            .transpose()
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn daemon_session(host: tauri::State<'_, Host>) -> Result<Session, String> {
    host.0
        .lock()
        .map_err(|_| "daemon host lock poisoned".to_string())?
        .session()
        .map_err(|e| e.to_string())
}
fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|arg| arg == "--verify-host") {
        let result = arguments
            .get(2)
            .context("--verify-host requires a report path")
            .and_then(|report| verify_host(std::path::Path::new(report)));
        if let Err(error) = result {
            eprintln!("Portable host verification failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    let builder = tauri::Builder::default()
        .manage(UnsavedChanges::default())
        .on_window_event(|window, event| {
            if window.label() == "main" && window.state::<UnsavedChanges>().0.load(Ordering::SeqCst)
            {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.emit("iris:close-requested", ());
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(not(feature = "native-smoke"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        frontend_ready,
        set_unsaved_changes,
        confirm_close,
        daemon_session,
        set_app_locale,
        select_project_folder,
        select_export_csv,
        updater::update_status,
        updater::check_for_update,
        updater::download_update,
        updater::install_update
    ]);
    #[cfg(feature = "native-smoke")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        frontend_ready,
        set_unsaved_changes,
        confirm_close,
        daemon_session,
        set_app_locale,
        select_project_folder,
        select_export_csv,
        updater::update_status,
        updater::check_for_update,
        updater::download_update,
        updater::install_update,
        native_smoke::native_title_smoke_observe,
        native_smoke::native_title_smoke_finish
    ]);
    #[cfg(feature = "native-smoke")]
    let builder = native_smoke::configure(builder).expect("configure native smoke test");
    let app = builder
        .setup(|app| {
            let executable = daemon_executable()?;
            let data_dir = std::env::var_os("IRIS_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            std::fs::create_dir_all(&data_dir)?;
            let mut host = DaemonHost::new(executable, data_dir);
            host.start()?;
            let host = Arc::new(Mutex::new(host));
            app.manage(Host(host.clone()));
            app.manage(updater::initialize(app.handle(), host.clone())?);
            #[cfg(feature = "native-smoke")]
            native_smoke::arm_timeout(app.handle().clone());
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(3));
                let Ok(mut host) = host.lock() else { break };
                match host.supervise() {
                    Ok(Some(_)) => {
                        let _ = handle.emit("daemon:restarted", ());
                    }
                    Err(_) => {
                        let _ = handle.emit("daemon:unavailable", ());
                    }
                    Ok(None) => {}
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to initialize desktop daemon host");
    app.run(|handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(host) = handle.try_state::<Host>() {
                if let Ok(mut host) = host.0.lock() {
                    host.stop();
                }
            }
        }
    });
}
