use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr::null_mut;
use std::time::Instant;

use crate::domain::exit_code::exit_code_reference_text;
use crate::domain::validator::normalize_for_compare;
use crate::infra::win32_ffi::*;

use super::APP_VERSION;
use super::theme::ThemeFonts;

const ID_EXEC_PROCEED: INT = 100;
const ID_SHUTDOWN_NOW: INT = 200;

fn resolve_existing_dir(raw: &str) -> Option<String> {
    let norm = normalize_for_compare(raw);
    if norm.is_empty() {
        return None;
    }
    let mut cur = PathBuf::from(&norm);
    while !cur.exists() {
        if !cur.pop() {
            return None;
        }
    }
    if cur.is_dir() {
        Some(cur.to_string_lossy().to_string())
    } else {
        cur.parent().map(|p| p.to_string_lossy().to_string())
    }
}

unsafe extern "system" fn browse_callback_proc(
    hwnd: HWND,
    msg: UINT,
    _lparam: LPARAM,
    lp_data: LPARAM,
) -> INT {
    if msg == BFFM_INITIALIZED && lp_data != 0 {
        unsafe {
            SendMessageW(hwnd, BFFM_SETSELECTIONW, TRUE as WPARAM, lp_data);
        }
    }
    0
}

/// フォルダ選択ダイアログを表示する（`initial_path` 指定時は該当または最深親フォルダを初期選択）。
pub fn pick_folder(hwnd: HWND, title: &str, initial_path: Option<&str>) -> Option<String> {
    let title_w = to_wide(title);
    let init_wide = initial_path
        .and_then(resolve_existing_dir)
        .map(|p| to_wide(&p));
    let mut display_buf = vec![0u16; 32768];
    let mut bi = BROWSEINFOW {
        hwndOwner: hwnd,
        pszDisplayName: display_buf.as_mut_ptr(),
        lpszTitle: title_w.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX,
        lpfn: if init_wide.is_some() {
            Some(browse_callback_proc)
        } else {
            None
        },
        lParam: init_wide.as_ref().map_or(0, |w| w.as_ptr() as LPARAM),
        ..Default::default()
    };

    unsafe {
        let co_hr = CoInitializeEx(null_mut(), COINIT_APARTMENTTHREADED);
        let pidl = SHBrowseForFolderW(&raw mut bi);
        if pidl.is_null() {
            if co_hr >= 0 {
                CoUninitialize();
            }
            return None;
        }
        let mut path_buf = vec![0u16; 32768];
        let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr());
        CoTaskMemFree(pidl);
        if co_hr >= 0 {
            CoUninitialize();
        }
        if ok == 0 {
            return None;
        }
        let path = from_wide_nul(&path_buf);
        if path.is_empty() { None } else { Some(path) }
    }
}

/// `.bat` 保存先ファイル選択ダイアログを表示する。
pub fn pick_save_bat_path(hwnd: HWND, default_name: &str) -> Option<PathBuf> {
    let filter: Vec<u16> = "バッチファイル (*.bat)\0*.bat\0すべてのファイル (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title_w = to_wide("バッチファイル(.bat)の出力先を指定");
    let def_ext_w = to_wide("bat");

    let mut file_buf = vec![0u16; 32768];
    let def_w: Vec<u16> = default_name.encode_utf16().collect();
    let copy_len = def_w.len().min(file_buf.len() - 1);
    file_buf[..copy_len].copy_from_slice(&def_w[..copy_len]);

    let mut ofn = OPENFILENAMEW {
        hwndOwner: hwnd,
        lpstrFilter: filter.as_ptr(),
        nFilterIndex: 1,
        lpstrFile: file_buf.as_mut_ptr(),
        nMaxFile: file_buf.len() as DWORD,
        lpstrTitle: title_w.as_ptr(),
        lpstrDefExt: def_ext_w.as_ptr(),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };

    let ok = unsafe { GetSaveFileNameW(&raw mut ofn) };
    if ok == 0 {
        return None;
    }
    let path = from_wide_nul(&file_buf);
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

pub fn show_info_box(hwnd: HWND, title: &str, message: &str) {
    show_simple_task_dialog(hwnd, title, title, message, TD_INFORMATION_ICON);
}

pub fn show_warning_box(hwnd: HWND, title: &str, message: &str) {
    show_simple_task_dialog(hwnd, title, title, message, TD_WARNING_ICON);
}

pub fn show_error_box(hwnd: HWND, title: &str, message: &str) {
    show_simple_task_dialog(hwnd, title, title, message, TD_ERROR_ICON);
}

pub fn confirm_yes_no(hwnd: HWND, title: &str, message: &str) -> bool {
    let t = to_wide(title);
    let m = to_wide(message);
    let res = unsafe {
        MessageBoxW(
            hwnd,
            m.as_ptr(),
            t.as_ptr(),
            MB_YESNO | MB_ICONEXCLAMATION | MB_DEFBUTTON2,
        )
    };
    res == IDYES
}

fn show_simple_task_dialog(
    hwnd: HWND,
    title: &str,
    instruction: &str,
    content: &str,
    icon: *const u16,
) {
    let title_w = to_wide(title);
    let inst_w = to_wide(instruction);
    let cont_w = to_wide(content);

    let cfg = TASKDIALOGCONFIG {
        hwndParent: hwnd,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_POSITION_RELATIVE_TO_WINDOW,
        dwCommonButtons: TDCBF_OK_BUTTON,
        pszWindowTitle: title_w.as_ptr(),
        hMainIcon: icon,
        pszMainInstruction: inst_w.as_ptr(),
        pszContent: cont_w.as_ptr(),
        ..Default::default()
    };

    let mut btn = 0;
    let hr = unsafe { TaskDialogIndirect(&raw const cfg, &raw mut btn, null_mut(), null_mut()) };
    if hr != 0 {
        let mb_icon = if icon == TD_ERROR_ICON {
            MB_ICONERROR
        } else if icon == TD_WARNING_ICON {
            MB_ICONEXCLAMATION
        } else {
            MB_ICONINFORMATION
        };
        unsafe {
            MessageBoxW(hwnd, cont_w.as_ptr(), title_w.as_ptr(), MB_OK | mb_icon);
        }
    }
}

pub fn show_exit_code_help(hwnd: HWND) {
    show_simple_task_dialog(
        hwnd,
        "robocopy 終了コードの解説",
        "終了コード一覧（0〜7: 正常系 / 8以上: 異常系）",
        exit_code_reference_text(),
        TD_INFORMATION_ICON,
    );
}

pub fn show_about_dialog(hwnd: HWND, config_path: &str, logs_dir: &str, is_elevated: bool) {
    let priv_label = if is_elevated {
        "管理者権限"
    } else {
        "通常権限"
    };
    let msg = format!(
        "Robocopy-GUI v{APP_VERSION}\r\n\
         作成者: NRO / Antigravity\r\n\
         実行権限: {priv_label}\r\n\r\n\
         Windows 標準 robocopy.exe 用の安全・軽量ポータブルGUIラッパーです。\r\n\
         レジストリを一切使用せず、設定とログを実行ファイル同階層に保持します。\r\n\r\n\
         設定ファイル: {config_path}\r\n\
         ログ出力先: {logs_dir}"
    );
    show_simple_task_dialog(
        hwnd,
        "バージョン情報",
        &format!("Robocopy-GUI v{APP_VERSION}"),
        &msg,
        TD_INFORMATION_ICON,
    );
}

/// 破壊的操作・完了後シャットダウン等の実行前確認ダイアログ（既定フォーカスはキャンセル）。
pub fn confirm_pre_execution(
    hwnd: HWND,
    destructive_summaries: &[String],
    command_previews: &[String],
    warnings: &[String],
    shutdown_enabled: bool,
) -> bool {
    let mut body_parts = Vec::new();
    if !destructive_summaries.is_empty() {
        body_parts.push("【注意】削除を伴う破壊的操作が含まれています:".to_string());
        for s in destructive_summaries {
            body_parts.push(format!("  - {s}"));
        }
    }
    if shutdown_enabled {
        body_parts.push(
            "【重要】完了後にPCをシャットダウンする設定がONです。\n\
             他アプリで編集中の未保存ファイルがある場合は、必ず事前に保存してください。\n\
             （テスト実行・ユーザー中断・終了コード8以上の異常時はシャットダウンしません）"
                .to_string(),
        );
    }
    if !warnings.is_empty() {
        body_parts.push("【補足・警告】".to_string());
        for w in warnings {
            body_parts.push(format!("  - {w}"));
        }
    }

    let content = body_parts.join("\n\n");
    let expanded = command_previews.join("\n");

    let title_w = to_wide("実行前確認 - Robocopy-GUI");
    let inst_w = to_wide("バックアップを実行してもよろしいですか？");
    let cont_w = to_wide(&content);
    let exp_w = to_wide(&expanded);
    let exp_lbl_w = to_wide("実行コマンド全文を表示");
    let col_lbl_w = to_wide("実行コマンド全文を隠す");
    let run_btn_w = to_wide("実行する");
    let cancel_btn_w = to_wide("キャンセル");

    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: ID_EXEC_PROCEED,
            pszButtonText: run_btn_w.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: IDCANCEL,
            pszButtonText: cancel_btn_w.as_ptr(),
        },
    ];

    let cfg = TASKDIALOGCONFIG {
        hwndParent: hwnd,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
            | TDF_EXPAND_FOOTER_AREA
            | TDF_POSITION_RELATIVE_TO_WINDOW,
        pszWindowTitle: title_w.as_ptr(),
        hMainIcon: TD_WARNING_ICON,
        pszMainInstruction: inst_w.as_ptr(),
        pszContent: cont_w.as_ptr(),
        cButtons: buttons.len() as UINT,
        pButtons: buttons.as_ptr(),
        nDefaultButton: IDCANCEL,
        pszExpandedInformation: exp_w.as_ptr(),
        pszExpandedControlText: col_lbl_w.as_ptr(),
        pszCollapsedControlText: exp_lbl_w.as_ptr(),
        ..Default::default()
    };

    let mut chosen = IDCANCEL;
    let hr = unsafe { TaskDialogIndirect(&raw const cfg, &raw mut chosen, null_mut(), null_mut()) };
    if hr != 0 {
        return confirm_yes_no(hwnd, "実行前確認", &content);
    }
    chosen == ID_EXEC_PROCEED
}

struct ShutdownCountdownCtx {
    started_at: Instant,
    last_tick_at: Instant,
    total_secs: u64,
    last_announced_sec: u64,
    cancelled_by_sleep_jump: bool,
}

unsafe extern "system" fn shutdown_td_callback(
    hwnd: HWND,
    msg: UINT,
    _wparam: WPARAM,
    _lparam: LPARAM,
    lp_ref_data: isize,
) -> HRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *(lp_ref_data as *mut ShutdownCountdownCtx) };
        match msg {
            TDN_CREATED => {
                let range = ((ctx.total_secs as usize) << 16) as LPARAM;
                unsafe {
                    SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_RANGE, 0, range);
                    SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_POS, ctx.total_secs as WPARAM, 0);
                }
            }
            TDN_TIMER => {
                let now = Instant::now();
                // スリープ復帰等でタイマー間隔が大きく飛んだ場合は安全のため自動中止する。
                if now.duration_since(ctx.last_tick_at).as_secs() > 5 {
                    ctx.cancelled_by_sleep_jump = true;
                    unsafe {
                        SendMessageW(hwnd, TDM_CLICK_BUTTON, IDCANCEL as WPARAM, 0);
                    }
                    return 0;
                }
                ctx.last_tick_at = now;

                let elapsed = now.duration_since(ctx.started_at).as_secs();
                let remaining = ctx.total_secs.saturating_sub(elapsed);

                if remaining != ctx.last_announced_sec {
                    ctx.last_announced_sec = remaining;
                    unsafe {
                        SendMessageW(hwnd, TDM_SET_PROGRESS_BAR_POS, remaining as WPARAM, 0);
                    }
                    let footer_str =
                        format!("残り {remaining} 秒でシャットダウンを実行します（Esc で中止）");
                    let footer_w = to_wide(&footer_str);
                    unsafe {
                        SendMessageW(
                            hwnd,
                            TDM_SET_ELEMENT_TEXT,
                            TDE_FOOTER,
                            footer_w.as_ptr() as LPARAM,
                        );
                    }
                    // NVDA読み上げ阻害を防ぐため、本文テキスト自体は開始・30秒・10秒の節目のみ更新する。
                    if matches!(remaining, 30 | 10 | 5) {
                        let body = format!(
                            "バックアップが正常に完了し、ログを保存しました。\n残り {remaining} 秒でPCをシャットダウンします。\n中止するには Esc キーまたは「シャットダウンを中止」を押してください。"
                        );
                        let body_w = to_wide(&body);
                        unsafe {
                            SendMessageW(
                                hwnd,
                                TDM_SET_ELEMENT_TEXT,
                                TDE_CONTENT,
                                body_w.as_ptr() as LPARAM,
                            );
                        }
                    }
                }

                if remaining == 0 {
                    unsafe {
                        SendMessageW(hwnd, TDM_CLICK_BUTTON, ID_SHUTDOWN_NOW as WPARAM, 0);
                    }
                }
            }
            TDN_BUTTON_CLICKED => {}
            _ => {}
        }
        0
    }))
    .unwrap_or(0)
}

/// 60秒カウントダウン付きシャットダウン確認モーダル（中止またはスリープ検知時は false）。
pub fn show_shutdown_countdown_dialog(hwnd: HWND) -> bool {
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
        MessageBeep(MB_ICONEXCLAMATION);
    }

    let mut ctx = ShutdownCountdownCtx {
        started_at: Instant::now(),
        last_tick_at: Instant::now(),
        total_secs: 60,
        last_announced_sec: 60,
        cancelled_by_sleep_jump: false,
    };

    let title_w = to_wide("バックアップ完了 - シャットダウン確認");
    let inst_w = to_wide("バックアップが正常に完了しました");
    let cont_w = to_wide(
        "ログファイルを保存しました。60秒後にPCをシャットダウンします。\n\
         シャットダウンを取り消す場合は Esc キーまたは「シャットダウンを中止」を押してください。",
    );
    let footer_w = to_wide("残り 60 秒でシャットダウンを実行します（Esc で中止）");
    let now_btn_w = to_wide("今すぐシャットダウン");
    let cancel_btn_w = to_wide("シャットダウンを中止 (Esc)");

    let buttons = [
        TASKDIALOG_BUTTON {
            nButtonID: IDCANCEL,
            pszButtonText: cancel_btn_w.as_ptr(),
        },
        TASKDIALOG_BUTTON {
            nButtonID: ID_SHUTDOWN_NOW,
            pszButtonText: now_btn_w.as_ptr(),
        },
    ];

    let cfg = TASKDIALOGCONFIG {
        hwndParent: hwnd,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
            | TDF_CALLBACK_TIMER
            | TDF_SHOW_PROGRESS_BAR
            | TDF_POSITION_RELATIVE_TO_WINDOW,
        pszWindowTitle: title_w.as_ptr(),
        hMainIcon: TD_WARNING_ICON,
        pszMainInstruction: inst_w.as_ptr(),
        pszContent: cont_w.as_ptr(),
        pszFooter: footer_w.as_ptr(),
        cButtons: buttons.len() as UINT,
        pButtons: buttons.as_ptr(),
        nDefaultButton: IDCANCEL,
        pfCallback: Some(shutdown_td_callback),
        lpCallbackData: (&raw mut ctx) as isize,
        ..Default::default()
    };

    let mut chosen = IDCANCEL;
    let hr = unsafe { TaskDialogIndirect(&raw const cfg, &raw mut chosen, null_mut(), null_mut()) };
    if hr != 0 || ctx.cancelled_by_sleep_jump {
        return false;
    }
    chosen == ID_SHUTDOWN_NOW
}

const INPUT_CLASS_NAME: &str = "RobocopyGuiPromptInputClass";
const IDC_INPUT_EDIT: usize = 301;

struct PromptDialogState {
    edit_hwnd: HWND,
    result: RefCell<Option<String>>,
    done: Cell<bool>,
}

unsafe extern "system" fn prompt_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    catch_unwind(AssertUnwindSafe(|| {
        match msg {
            WM_COMMAND => {
                let id = wparam & 0xFFFF;
                let state_ptr =
                    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const PromptDialogState;
                if !state_ptr.is_null() {
                    let state = unsafe { &*state_ptr };
                    if id == IDOK as usize {
                        let txt = get_window_text(state.edit_hwnd);
                        *state.result.borrow_mut() = Some(txt);
                        state.done.set(true);
                        return 0;
                    } else if id == IDCANCEL as usize {
                        *state.result.borrow_mut() = None;
                        state.done.set(true);
                        return 0;
                    }
                }
            }
            WM_CLOSE => {
                let state_ptr =
                    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const PromptDialogState;
                if !state_ptr.is_null() {
                    let state = unsafe { &*state_ptr };
                    *state.result.borrow_mut() = None;
                    state.done.set(true);
                }
                return 0;
            }
            WM_DESTROY => {
                let state_ptr =
                    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const PromptDialogState;
                if !state_ptr.is_null() {
                    let state = unsafe { &*state_ptr };
                    state.done.set(true);
                }
                return 0;
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }))
    .unwrap_or(0)
}

/// プリセット新規保存用のモーダルテキスト入力ダイアログ。
pub fn prompt_preset_name(parent: HWND, initial: &str) -> Option<String> {
    let h_inst = unsafe { GetModuleHandleW(null_mut()) };
    let class_w = to_wide(INPUT_CLASS_NAME);
    let wc = WNDCLASSEXW {
        lpfnWndProc: Some(prompt_wnd_proc),
        hInstance: h_inst,
        hCursor: unsafe { LoadCursorW(0, IDC_ARROW as *const u16) },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_BTNFACE) },
        lpszClassName: class_w.as_ptr(),
        ..Default::default()
    };
    unsafe {
        RegisterClassExW(&raw const wc);
    }

    let dpi = unsafe { GetDpiForWindow(parent) }.max(96);
    let fonts = ThemeFonts::for_dpi(dpi);
    let w = scale_dpi(400, dpi);
    let h = scale_dpi(155, dpi);

    let mut parent_rect = RECT::default();
    unsafe { GetWindowRect(parent, &raw mut parent_rect) };
    let x = parent_rect.left + ((parent_rect.right - parent_rect.left - w) / 2);
    let y = parent_rect.top + ((parent_rect.bottom - parent_rect.top - h) / 2);

    let title_w = to_wide("プリセットの新規保存");
    let dlg = unsafe {
        CreateWindowExW(
            WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT,
            class_w.as_ptr(),
            title_w.as_ptr(),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            w,
            h,
            parent,
            0,
            h_inst,
            null_mut(),
        )
    };
    if dlg == 0 {
        return None;
    }
    apply_rounded_corners(dlg);

    let static_cls = to_wide("STATIC");
    let edit_cls = to_wide("EDIT");
    let btn_cls = to_wide("BUTTON");
    let lbl_w = to_wide("新しいプリセット名(&N):");
    let init_w = to_wide(initial);
    let ok_w = to_wide("保存");
    let cancel_w = to_wide("キャンセル");

    let lbl = unsafe {
        CreateWindowExW(
            0,
            static_cls.as_ptr(),
            lbl_w.as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_LEFT,
            scale_dpi(14, dpi),
            scale_dpi(14, dpi),
            scale_dpi(360, dpi),
            scale_dpi(20, dpi),
            dlg,
            0,
            h_inst,
            null_mut(),
        )
    };
    let edit = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_cls.as_ptr(),
            init_w.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL,
            scale_dpi(14, dpi),
            scale_dpi(38, dpi),
            scale_dpi(356, dpi),
            scale_dpi(26, dpi),
            dlg,
            IDC_INPUT_EDIT as HMENU,
            h_inst,
            null_mut(),
        )
    };
    let btn_ok = unsafe {
        CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            ok_w.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON,
            scale_dpi(186, dpi),
            scale_dpi(78, dpi),
            scale_dpi(88, dpi),
            scale_dpi(28, dpi),
            dlg,
            IDOK as HMENU,
            h_inst,
            null_mut(),
        )
    };
    let btn_cancel = unsafe {
        CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            cancel_w.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON,
            scale_dpi(282, dpi),
            scale_dpi(78, dpi),
            scale_dpi(88, dpi),
            scale_dpi(28, dpi),
            dlg,
            IDCANCEL as HMENU,
            h_inst,
            null_mut(),
        )
    };

    for ctrl in [lbl, edit, btn_ok, btn_cancel] {
        unsafe {
            SendMessageW(ctrl, WM_SETFONT, fonts.ui_font.0 as WPARAM, 1);
        }
    }

    let state = PromptDialogState {
        edit_hwnd: edit,
        result: RefCell::new(None),
        done: Cell::new(false),
    };
    unsafe {
        SetWindowLongPtrW(dlg, GWLP_USERDATA, (&raw const state) as isize);
        EnableWindow(parent, FALSE);
        ShowWindow(dlg, SW_SHOW);
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);
    }

    let mut msg = MSG::default();
    while !state.done.get() {
        let gm = unsafe { GetMessageW(&raw mut msg, 0, 0, 0) };
        if gm == 0 {
            unsafe {
                PostQuitMessage(msg.wParam as INT);
            }
            break;
        }
        if gm < 0 {
            break;
        }
        if unsafe { IsDialogMessageW(dlg, &raw const msg) } == 0 {
            unsafe {
                TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }
        }
    }

    unsafe {
        EnableWindow(parent, TRUE);
        SetWindowLongPtrW(dlg, GWLP_USERDATA, 0);
        DestroyWindow(dlg);
        SetForegroundWindow(parent);
    }
    state.result.into_inner()
}
