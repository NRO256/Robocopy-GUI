use crate::domain::profile::BackupProfile;
use crate::infra::win32_ffi::*;

pub const IDM_FILE_SAVE: UINT = 1001;
pub const IDM_FILE_EXPORT_BAT: UINT = 1002;
pub const IDM_FILE_OPEN_LOGS: UINT = 1003;
pub const IDM_FILE_EXIT: UINT = 1004;
pub const IDM_TOOL_RESTART_ADMIN: UINT = 1010;
pub const IDM_HELP_EXIT_CODES: UINT = 1020;
pub const IDM_HELP_ABOUT: UINT = 1021;

pub const IDM_PRESET_RUN_BASE: UINT = 1100;
pub const MAX_PRESET_MENU_ITEMS: UINT = 100;

#[derive(Debug, Clone, Copy)]
pub struct MenuHandles {
    pub bar: HMENU,
    pub preset_menu: HMENU,
}

/// プリセット実行メニュー用の表示ラベルを生成する（& エスケープと先頭9件のアクセスキー付与）。
pub fn format_preset_menu_label(idx: usize, name: &str) -> String {
    let escaped = name.replace('&', "&&");
    if idx < 9 {
        format!("&{} {escaped}", idx + 1)
    } else {
        escaped
    }
}

/// メニューバーを構築しウィンドウへ設定する（アクセスキー &F, &R, &T, &H 予約）。
pub fn build_main_menu(hwnd: HWND, is_elevated: bool) -> MenuHandles {
    let bar = unsafe { CreateMenu() };
    let file_menu = unsafe { CreatePopupMenu() };
    let preset_menu = unsafe { CreatePopupMenu() };
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
    let r_w = to_wide("プリセット実行(&R)");
    let t_w = to_wide("ツール(&T)");
    let h_w = to_wide("ヘルプ(&H)");
    unsafe {
        AppendMenuW(bar, MF_POPUP, file_menu as usize, f_w.as_ptr());
        AppendMenuW(bar, MF_POPUP, preset_menu as usize, r_w.as_ptr());
        AppendMenuW(bar, MF_POPUP, tool_menu as usize, t_w.as_ptr());
        AppendMenuW(bar, MF_POPUP, help_menu as usize, h_w.as_ptr());
        SetMenu(hwnd, bar);
    }

    let handles = MenuHandles { bar, preset_menu };
    sync_menu_state(hwnd, handles, is_elevated, false);
    handles
}

/// プリセット実行メニューの内容を最新のプリセット一覧で再構築する。
pub fn sync_preset_run_menu(
    hwnd: HWND,
    preset_menu: HMENU,
    presets: &[BackupProfile],
    is_busy: bool,
) {
    if preset_menu == 0 {
        return;
    }
    unsafe {
        let count = GetMenuItemCount(preset_menu);
        for _ in 0..count {
            DeleteMenu(preset_menu, 0, MF_BYPOSITION);
        }

        if presets.is_empty() {
            let empty_w = to_wide("(プリセットなし)");
            AppendMenuW(preset_menu, MF_STRING | MF_GRAYED, 0, empty_w.as_ptr());
        } else {
            let item_state = if is_busy { MF_GRAYED } else { MF_ENABLED };
            for (idx, preset) in presets
                .iter()
                .take(MAX_PRESET_MENU_ITEMS as usize)
                .enumerate()
            {
                let label = format_preset_menu_label(idx, &preset.name);
                let w = to_wide(&label);
                let id = (IDM_PRESET_RUN_BASE + idx as UINT) as usize;
                AppendMenuW(preset_menu, MF_STRING | item_state, id, w.as_ptr());
            }
        }
        DrawMenuBar(hwnd);
    }
}

/// 管理者権限および実行中状態に応じてメニュー項目の有効/チェック状態を更新する。
pub fn sync_menu_state(hwnd: HWND, menus: MenuHandles, is_elevated: bool, is_busy: bool) {
    if menus.bar == 0 {
        return;
    }
    unsafe {
        if is_elevated {
            EnableMenuItem(menus.bar, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | MF_GRAYED);
            CheckMenuItem(menus.bar, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | MF_CHECKED);
        } else {
            let state = if is_busy { MF_GRAYED } else { MF_ENABLED };
            EnableMenuItem(menus.bar, IDM_TOOL_RESTART_ADMIN, MF_BYCOMMAND | state);
            CheckMenuItem(
                menus.bar,
                IDM_TOOL_RESTART_ADMIN,
                MF_BYCOMMAND | MF_UNCHECKED,
            );
        }
        let file_state = if is_busy { MF_GRAYED } else { MF_ENABLED };
        EnableMenuItem(menus.bar, IDM_FILE_SAVE, MF_BYCOMMAND | file_state);
        EnableMenuItem(menus.bar, IDM_FILE_EXPORT_BAT, MF_BYCOMMAND | file_state);

        if menus.preset_menu != 0 {
            let count = GetMenuItemCount(menus.preset_menu).max(0) as UINT;
            for idx in 0..count {
                EnableMenuItem(
                    menus.preset_menu,
                    IDM_PRESET_RUN_BASE + idx,
                    MF_BYCOMMAND | file_state,
                );
            }
        }
        DrawMenuBar(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_menu_label_escapes_ampersand_and_adds_shortcut() {
        assert_eq!(format_preset_menu_label(0, "Daily"), "&1 Daily");
        assert_eq!(format_preset_menu_label(8, "A&B"), "&9 A&&B");
        assert_eq!(format_preset_menu_label(9, "Tenth"), "Tenth");
    }
}
