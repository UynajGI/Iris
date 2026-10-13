use iris_shell::diagnostics::{self, Event};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

pub struct SupportPaths(pub PathBuf);

#[derive(Serialize)]
pub struct SupportInfo {
    version: &'static str,
    build: &'static str,
    platform: &'static str,
    architecture: &'static str,
    distribution: &'static str,
    debug: bool,
    logs_available: bool,
    daemon_version: Option<String>,
    data_directory: String,
    log_directory: String,
}

fn main_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("仅主窗口可使用帮助与诊断".into())
    }
}

#[tauri::command]
pub fn support_info(
    window: tauri::WebviewWindow,
    paths: tauri::State<'_, SupportPaths>,
) -> Result<SupportInfo, String> {
    main_window(&window)?;
    Ok(SupportInfo {
        version: env!("CARGO_PKG_VERSION"),
        build: env!("IRIS_BUILD_ID"),
        platform: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        distribution: option_env!("IRIS_DISTRIBUTION").unwrap_or("portable"),
        debug: cfg!(debug_assertions),
        logs_available: diagnostics::available(),
        daemon_version: window
            .app_handle()
            .try_state::<crate::Host>()
            .and_then(|host| {
                host.0
                    .try_lock()
                    .ok()
                    .and_then(|host| host.session().ok())
                    .map(|session| session.version)
            }),
        data_directory: paths.0.to_string_lossy().into_owned(),
        log_directory: paths.0.join("logs").to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn record_diagnostic_event(window: tauri::WebviewWindow, event: Event) -> Result<(), String> {
    main_window(&window)?;
    diagnostics::record(event);
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportLink {
    Homepage,
    Releases,
    Issues,
    Logs,
}

#[tauri::command]
pub async fn open_support_link(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    target: SupportLink,
) -> Result<(), String> {
    main_window(&window)?;
    let value = match target {
        SupportLink::Homepage => "https://github.com/UynajGI/Iris".into(),
        SupportLink::Releases => "https://github.com/UynajGI/Iris/releases".into(),
        SupportLink::Issues => "https://github.com/UynajGI/Iris/issues/new/choose".into(),
        SupportLink::Logs => app.state::<SupportPaths>().0.join("logs").into_os_string(),
    };
    tauri::async_runtime::spawn_blocking(move || open_system_target(value))
        .await
        .map_err(|_| "打开操作未能完成".to_string())?
}

#[tauri::command]
pub async fn open_export_folder(
    window: tauri::WebviewWindow,
    path: String,
    file: bool,
) -> Result<(), String> {
    main_window(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory =
            iris_shell::export_location::export_folder(std::path::Path::new(&path), file)?;
        open_system_target(directory.into_os_string())
    })
    .await
    .map_err(|_| "打开文件夹未能完成".to_string())?
}

fn open_system_target(value: std::ffi::OsString) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("explorer.exe");
        command.creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    // Only fixed support links or validated absolute directories reach the launcher.
    let mut child = command
        .arg(value)
        .spawn()
        .map_err(|_| "无法打开，请检查系统默认浏览器或文件管理器".to_string())?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
pub async fn export_diagnostics(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<bool, String> {
    main_window(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app.dialog().file().add_filter("诊断信息", &["json"])
            .set_file_name("Iris-diagnostics.json").blocking_save_file() else { return Ok(false) };
        let path = file.into_path().map_err(|_| "请选择本地文件".to_string())?;
        if !path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("json")) {
            return Err("诊断信息须保存为 .json 文件".into());
        }
        let entries = diagnostics::snapshot().map_err(|_| "无法读取日志，请检查磁盘空间与权限".to_string())?;
        let report = serde_json::json!({
            "schema": 1, "product": "Iris", "version": env!("CARGO_PKG_VERSION"),
            "build": env!("IRIS_BUILD_ID"), "platform": std::env::consts::OS,
            "architecture": std::env::consts::ARCH, "events": entries,
            "daemon_available": app.try_state::<crate::Host>().is_some_and(|host| host.0.try_lock().is_ok_and(|host| host.session().is_ok())),
            "privacy": "Event codes only; no photographs, paths, database, credentials or raw stderr."
        });
        let file = std::fs::OpenOptions::new().write(true).create_new(true).open(path)
            .map_err(|_| "无法保存：请选择新文件名，并检查目录权限和磁盘空间".to_string())?;
        serde_json::to_writer_pretty(file, &report).map_err(|_| "写入诊断信息失败，请检查磁盘空间".to_string())?;
        diagnostics::record(Event::DiagnosticExported);
        Ok(true)
    }).await.map_err(|_| "诊断导出未能完成".to_string())?
}
