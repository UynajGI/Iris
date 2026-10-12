//! Keep Windows' real caption buttons while sharing their row with the app toolbar.
//! Other platforms retain their native window decoration.
#[cfg(windows)]
mod windows {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Dwm::{DwmDefWindowProc, DwmExtendFrameIntoClientArea},
        UI::{
            Controls::MARGINS,
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    };

    unsafe fn extend(hwnd: HWND) {
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: (60 * GetDpiForWindow(hwnd) / 96) as i32,
            cyBottomHeight: 0,
        };
        DwmExtendFrameIntoClientArea(hwnd, &margins);
    }

    unsafe extern "system" fn frame_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if message == WM_NCCALCSIZE && wparam != 0 {
            let params = &mut *(lparam as *mut NCCALCSIZE_PARAMS);
            let top = params.rgrc[0].top;
            DefSubclassProc(hwnd, message, wparam, lparam);
            // Preserve native side/bottom resize borders and the maximized work area.
            let dpi = GetDpiForWindow(hwnd);
            let inset = if IsZoomed(hwnd) != 0 {
                GetSystemMetricsForDpi(SM_CYFRAME, dpi)
                    + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
            } else {
                0
            };
            params.rgrc[0].top = top + inset;
            return 0;
        }
        let mut result = 0;
        if DwmDefWindowProc(hwnd, message, wparam, lparam, &mut result) != 0 {
            return result;
        }
        if message == WM_ACTIVATE || message == WM_DPICHANGED {
            extend(hwnd);
        }
        if message == WM_NCHITTEST {
            let mut rect: RECT = std::mem::zeroed();
            GetWindowRect(hwnd, &mut rect);
            let y = ((lparam >> 16) as i16) as i32;
            let dpi = GetDpiForWindow(hwnd);
            let border = GetSystemMetricsForDpi(SM_CYFRAME, dpi)
                + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            result = DefSubclassProc(hwnd, message, wparam, lparam);
            if IsZoomed(hwnd) == 0 && y < rect.top + border {
                return match result as u32 {
                    HTLEFT | HTTOPLEFT => HTTOPLEFT as isize,
                    HTRIGHT | HTTOPRIGHT => HTTOPRIGHT as isize,
                    _ => HTTOP as isize,
                };
            }
            // Toolbar controls receive client input; its empty space uses Tauri dragging.
            if result == HTCAPTION as isize {
                return HTCLIENT as isize;
            }
            return result;
        }
        DefSubclassProc(hwnd, message, wparam, lparam)
    }

    pub fn install(window: &tauri::WebviewWindow) -> anyhow::Result<()> {
        let hwnd = window.hwnd()?.0 as HWND;
        // SAFETY: setup runs on the window's UI thread; this static callback never
        // retains Rust references, and Windows removes the subclass on destruction.
        unsafe {
            anyhow::ensure!(
                SetWindowSubclass(hwnd, Some(frame_proc), 1, 0) != 0,
                "install native window frame failed"
            );
            extend(hwnd);
            anyhow::ensure!(
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED
                ) != 0,
                "refresh native window frame failed"
            );
        }
        Ok(())
    }
}

pub fn setup(app: &tauri::AppHandle) -> anyhow::Result<()> {
    use tauri::Manager;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| anyhow::anyhow!("main window missing"))?;
    window.set_theme(Some(tauri::Theme::Dark))?;
    #[cfg(windows)]
    {
        // An opaque WebView2 surface otherwise covers DWM's real caption buttons.
        // CSS leaves only their reserved corner transparent; the native layer stays dark.
        window.set_background_color(Some(tauri::utils::config::Color(18, 18, 20, 0)))?;
        windows::install(&window)?;
    }
    Ok(())
}
