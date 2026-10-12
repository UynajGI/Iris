use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum CloseBehavior {
    #[default]
    Ask,
    Background,
    Exit,
}

pub struct Lifecycle {
    path: PathBuf,
    behavior: Mutex<CloseBehavior>,
}

impl Lifecycle {
    fn load(path: PathBuf) -> Self {
        let behavior = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            behavior: Mutex::new(behavior),
        }
    }

    fn save(&self, behavior: CloseBehavior) -> Result<(), String> {
        let mut value = self.behavior.lock().map_err(|e| e.to_string())?;
        std::fs::write(
            &self.path,
            serde_json::to_vec(&behavior).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        *value = behavior;
        Ok(())
    }
}

pub fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn request_close(app: &tauri::AppHandle, force_exit: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let dirty = window
        .state::<super::UnsavedChanges>()
        .0
        .load(std::sync::atomic::Ordering::SeqCst);
    let behavior = *window.state::<Lifecycle>().behavior.lock().unwrap();
    if !force_exit && behavior == CloseBehavior::Background {
        if let Err(error) = window.hide() {
            let _ = window.emit("iris:window-error", error.to_string());
        }
    } else if force_exit || dirty || behavior == CloseBehavior::Ask {
        show_main(window.app_handle());
        let _ = window.emit(
            "iris:close-requested",
            serde_json::json!({ "forceExit": force_exit || behavior == CloseBehavior::Exit }),
        );
    } else {
        window.app_handle().exit(0);
    }
}

pub fn request_exit(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window
            .state::<super::UnsavedChanges>()
            .0
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            request_close(app, true);
            return;
        }
    }
    app.exit(0);
}

pub fn setup(app: &tauri::AppHandle, data_dir: PathBuf) -> anyhow::Result<()> {
    app.manage(Lifecycle::load(data_dir.join("window-close.json")));
    let open = MenuItem::with_id(app, "open", "打开伊人 / Open Iris", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit", "退出 / Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &exit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
    TrayIconBuilder::with_id("iris")
        .icon(icon)
        .tooltip("伊人 / Iris")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "exit" => request_exit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main_only(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the main window can manage closing".into());
    }
    Ok(())
}

#[tauri::command]
pub fn close_behavior(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Lifecycle>,
) -> Result<CloseBehavior, String> {
    main_only(&window)?;
    state
        .behavior
        .lock()
        .map(|value| *value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_close_behavior(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Lifecycle>,
    behavior: CloseBehavior,
) -> Result<(), String> {
    main_only(&window)?;
    state.save(behavior)
}

#[tauri::command]
pub fn confirm_close(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Lifecycle>,
    action: CloseBehavior,
    remember: bool,
) -> Result<(), String> {
    main_only(&window)?;
    if action == CloseBehavior::Ask {
        return Err("Choose background or exit".into());
    }
    if remember {
        state.save(action)?;
    }
    if action == CloseBehavior::Background {
        window.hide().map_err(|e| e.to_string())
    } else {
        window.app_handle().exit(0);
        Ok(())
    }
}

#[tauri::command]
pub fn window_frame_platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else {
        "native"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_preference_survives_restart_and_corruption_defaults_to_asking() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("iris-close-{}-{unique}.json", std::process::id()));
        let lifecycle = Lifecycle::load(path.clone());
        assert_eq!(*lifecycle.behavior.lock().unwrap(), CloseBehavior::Ask);
        for behavior in [
            CloseBehavior::Background,
            CloseBehavior::Exit,
            CloseBehavior::Ask,
        ] {
            lifecycle.save(behavior).unwrap();
            assert_eq!(
                *Lifecycle::load(path.clone()).behavior.lock().unwrap(),
                behavior
            );
        }
        std::fs::write(&path, b"broken preference").unwrap();
        assert_eq!(
            *Lifecycle::load(path.clone()).behavior.lock().unwrap(),
            CloseBehavior::Ask
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_preference_write_preserves_current_choice() {
        let lifecycle = Lifecycle::load(std::env::temp_dir());
        assert!(lifecycle.save(CloseBehavior::Exit).is_err());
        assert_eq!(*lifecycle.behavior.lock().unwrap(), CloseBehavior::Ask);
        assert!(serde_json::from_str::<CloseBehavior>("\"invalid\"").is_err());
    }
}
