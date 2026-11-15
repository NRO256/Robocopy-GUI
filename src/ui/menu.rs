use crate::infra::win32_ffi::*;

pub const IDM_FILE_SAVE: UINT = 1001;
pub const IDM_FILE_EXPORT_BAT: UINT = 1002;
pub const IDM_FILE_OPEN_LOGS: UINT = 1003;
pub const IDM_FILE_EXIT: UINT = 1004;
pub const IDM_TOOL_RESTART_ADMIN: UINT = 1010;
pub const IDM_HELP_EXIT_CODES: UINT = 1020;
pub const IDM_HELP_ABOUT: UINT = 1021;

/// メニューバーを構築しウィンドウへ設定する（アクセスキー &F, &T, &H 予約）。
pub fn build_main_menu(hwnd: HWND, is_elevated: bool) -> HMENU {
    let bar = unsafe { CreateMenu() };
    let file_menu = unsafe { CreatePopupMenu() };
    let tool_menu = unsafe { CreatePopupMenu() };
    let help_menu = unsafe { CreatePopupMenu() };

    let append = |menu: HMENU, flags: UINT, id: UINT, label: &str| {
        let w = to_wide(label);
        unsafe {
            AppendMenuW(menu, flags, id as usize, w.as_ptr());
        }
    };

    append(
        file_menu,
        MF_STRING,
        IDM_FILE_SAVE,
        "設定を保存(&S)\tCtrl+S",
    );
    append(
        file_menu,
        MF_STRING,
        IDM_FILE_EXPORT_BAT,
        "バッチファイル(.bat)出力(&E)...\tCtrl+E",
    );
    append(
        file_menu,
        MF_STRING,
        IDM_FILE_OPEN_LOGS,
        "ログフォルダを開く(&L)",
    );
    unsafe {
        AppendMenuW(file_menu, MF_SEPARATOR, 0, std::ptr::null());
    }
    append(file_menu, MF_STRING, IDM_FILE_EXIT, "終了(&X)");

    append(
        tool_menu,
        MF_STRING,
        IDM_TOOL_RESTART_ADMIN,
        "管理者として再起動(&A)",
    );
    append(
        help_menu,
        MF_STRING,
        IDM_HELP_EXIT_CODES,
        "終了コードの解説(&C)",
    );
    append(help_menu, MF_STRING, IDM_HELP_ABOUT, "バージョン情報(&V)");

    let f_w = to_wide("ファイル(&F)");
    let t_w = to_wide("ツール(&T)");
    let h_w = to_wide("ヘルプ(&H)");
    unsafe {
        AppendMenuW(bar, MF_POPUP, file_menu as usize, f_w.as_ptr());
        AppendMenuW(bar, MF_POPUP, tool_menu as usize, t_w.as_ptr());
        AppendMenuW(bar, MF_POPUP, help_menu as usize, h_w.as_ptr());
        SetMenu(hwnd, bar);
    }

    sync_menu_state(hwnd, bar, is_elevated, false);
    bar
}

/// 管理者権限および実行中状態に応じてメニュー項目の有効/チェック状態を更新する。
pub fn sync_menu_state(hwnd: HWND, menu: HMENU, is_elevated: bool, is_busy: bool) {
    if menu == 0 {
        return;
    }
    unsafe {
        if is_elevated {
            EnableMenuItem(menu, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | MF_GRAYED);
            CheckMenuItem(menu, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | MF_CHECKED);
        } else {
            let state = if is_busy { MF_GRAYED } else { MF_ENABLED };
            EnableMenuItem(menu, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | state);
            CheckMenuItem(menu, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | MF_UNCHECKED);
        }
        let file_state = if is_busy { MF_GRAYED } else { MF_ENABLED };
        EnableMenuItem(menu, IDM_FILE_SAVE, MF_BYCOMMAND | file_state);
        EnableMenuItem(menu, IDM_FILE_EXPORT_BAT, MF_BYCOMMAND | file_state);
        DrawMenuBar(hwnd);
    }
}
