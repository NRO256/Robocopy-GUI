pub(crate) mod dialogs;
pub(crate) mod menu;
pub(crate) mod theme;
pub(crate) mod window;

use std::ptr::null_mut;

use crate::app::AppState;
use crate::infra::storage::{PortablePaths, load_and_cleanup_handoff, load_config};
use crate::infra::system::is_process_elevated;
use crate::infra::win32_ffi::*;

use window::{APP_STATE, main_wnd_proc};

pub const WINDOW_CLASS_NAME: &str = "RobocopyGuiMainWindowClass";
pub const APP_VERSION: &str = "0.1.0";

pub fn run_gui(handoff_path: Option<std::path::PathBuf>, is_restarted: bool) -> Result<(), String> {
    let is_handoff = is_restarted || handoff_path.is_some();
    let mutex_name = to_wide("Local\\RobocopyGuiSingleInstanceMutex");
    let h_mutex = unsafe { CreateMutexW(null_mut(), 0, mutex_name.as_ptr()) };
    let _mutex_guard = HandleGuard(h_mutex);

    // 昇格再起動（--handoff）でない通常起動時は既存ウィンドウを前面化する。
    if !is_handoff && h_mutex != 0 && unsafe { GetLastError() == ERROR_ALREADY_EXISTS } {
        let cls = to_wide(WINDOW_CLASS_NAME);
        let existing = unsafe { FindWindowW(cls.as_ptr(), std::ptr::null()) };
        if existing != 0 {
            unsafe {
                if IsIconic(existing) != 0 {
                    ShowWindow(existing, SW_RESTORE);
                }
                SetForegroundWindow(existing);
            }
            return Ok(());
        }
    }

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as DWORD,
            dwICC: ICC_STANDARD_CLASSES | ICC_LISTVIEW_CLASSES | ICC_TAB_CLASSES,
        };
        InitCommonControlsEx(&raw const icc);
    }

    let paths = PortablePaths::resolve();
    let (config, load_warn) = if let Some(hp) = handoff_path.as_deref() {
        if let Some(handoff_cfg) = load_and_cleanup_handoff(hp) {
            (handoff_cfg, None)
        } else {
            load_config(&paths.config_path)
        }
    } else {
        load_config(&paths.config_path)
    };

    let elevated = is_process_elevated();
    let title = if elevated {
        format!("Robocopy-GUI v{APP_VERSION} [管理者]")
    } else {
        format!("Robocopy-GUI v{APP_VERSION}")
    };

    APP_STATE.with(|cell| {
        *cell.borrow_mut() = Some(AppState::new(config, paths, elevated));
    });

    let h_inst = unsafe { GetModuleHandleW(null_mut()) };
    let class_w = to_wide(WINDOW_CLASS_NAME);
    let title_w = to_wide(&title);

    let wc = WNDCLASSEXW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(main_wnd_proc),
        hInstance: h_inst,
        hIcon: unsafe { LoadIconW(0, IDI_APPLICATION as *const u16) },
        hCursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        lpszClassName: class_w.as_ptr(),
        hIconSm: unsafe { LoadIconW(0, IDI_APPLICATION as *const u16) },
        ..Default::default()
    };

    if unsafe { RegisterClassExW(&raw const wc) } == 0 {
        return Err("ウィンドウクラスの登録に失敗しました。".to_string());
    }

    let sys_dpi = unsafe { GetDpiForSystem() }.max(96);
    let init_w = scale_dpi(960, sys_dpi);
    let init_h = scale_dpi(720, sys_dpi);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class_w.as_ptr(),
            title_w.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            init_w,
            init_h,
            0,
            0,
            h_inst,
            null_mut(),
        )
    };

    if hwnd == 0 {
        return Err("メインウィンドウの作成に失敗しました。".to_string());
    }

    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
    }

    if let Some(warn) = load_warn {
        dialogs::show_warning_box(hwnd, "設定読み込みの注意", &warn);
    }

    let mut msg = MSG::default();
    loop {
        let gm = unsafe { GetMessageW(&raw mut msg, 0, 0, 0) };
        if gm <= 0 {
            break;
        }
        if msg.message == WM_KEYDOWN && unsafe { GetKeyState(VK_CONTROL) } < 0 {
            let key = msg.wParam as u8;
            if key == b'S' {
                unsafe {
                    PostMessageW(hwnd, WM_COMMAND, menu::IDM_FILE_SAVE as WPARAM, 0);
                }
                continue;
            }
            if key == b'E' {
                unsafe {
                    PostMessageW(hwnd, WM_COMMAND, menu::IDM_FILE_EXPORT_BAT as WPARAM, 0);
                }
                continue;
            }
        }
        if unsafe { IsDialogMessageW(hwnd, &raw const msg) } == 0 {
            unsafe {
                TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }
        }
    }

    Ok(())
}

/// 致命的エラー時のシンプルなメッセージボックス表示。
pub fn show_fatal_error(title: &str, message: &str) {
    let t = to_wide(title);
    let m = to_wide(message);
    unsafe {
        MessageBoxW(0, m.as_ptr(), t.as_ptr(), MB_OK | MB_ICONERROR);
    }
}
