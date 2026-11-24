use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::ptr::null_mut;

use crate::app::{AppState, RunPhase};
use crate::domain::command::{
    build_bat_script, has_exclusion_token, parse_xd_items, parse_xf_items, toggle_exclusion_token,
};
use crate::domain::profile::{BackupPrivilegeMode, BackupProfile, CopyMode};
use crate::domain::validator::FocusField;
use crate::infra::storage::{export_bat_file, save_config, write_handoff_config};
use crate::infra::system::{
    describe_paths_status, detect_mapped_drives, execute_system_shutdown,
    open_folder_or_parent_in_explorer, restart_as_admin,
};
use crate::infra::win32_ffi::*;

use super::dialogs::{
    ShutdownDialogOutcome, confirm_pre_execution, confirm_yes_no, pick_folder, pick_save_bat_path,
    prompt_preset_name, show_about_dialog, show_error_box, show_exit_code_help, show_info_box,
    show_shutdown_countdown_dialog, show_warning_box,
};
use super::menu::*;
use super::theme::{
    COLOR_CODE_BG, COLOR_TAB_BG, COLOR_TEXT_DANGER, COLOR_TEXT_INFO, COLOR_TEXT_OK, ThemeFonts,
};

pub const WM_APP_RUNNER_EVENT: UINT = WM_APP + 1;
pub const WM_APP_SWITCH_TAB: UINT = WM_APP + 2;
pub const WM_APP_CYCLE_TAB: UINT = WM_APP + 3;
const TIMER_ID_STATUS: usize = 1;
const MAX_GUI_LOG_LINES: i32 = 2000;

thread_local! {
    pub static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    static UI_HANDLES: RefCell<Option<UiHandles>> = const { RefCell::new(None) };
}

fn with_ui<R>(f: impl FnOnce(&UiHandles) -> R) -> Option<R> {
    UI_HANDLES.with(|cell| cell.try_borrow().ok().and_then(|g| g.as_ref().map(f)))
}

fn with_ui_mut<R>(f: impl FnOnce(&mut UiHandles) -> R) -> Option<R> {
    UI_HANDLES.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut g| g.as_mut().map(f))
    })
}

fn with_state<R>(f: impl FnOnce(&AppState) -> R) -> Option<R> {
    APP_STATE.with(|cell| cell.try_borrow().ok().and_then(|g| g.as_ref().map(f)))
}

fn with_state_mut<R>(f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    APP_STATE.with(|cell| {
        cell.try_borrow_mut()
            .ok()
            .and_then(|mut g| g.as_mut().map(f))
    })
}

fn with_ui_and_state<R>(f: impl FnOnce(&UiHandles, &AppState) -> R) -> Option<R> {
    with_ui(|h| with_state(|s| f(h, s))).flatten()
}

fn with_ui_and_state_mut<R>(f: impl FnOnce(&mut UiHandles, &mut AppState) -> R) -> Option<R> {
    with_ui_mut(|h| with_state_mut(|s| f(h, s))).flatten()
}

// コントロールID定義
const IDC_COMBO_PRESET: usize = 2001;
const IDC_BTN_PRESET_NEW: usize = 2002;
const IDC_BTN_PRESET_SAVE: usize = 2003;
const IDC_BTN_PRESET_DEL: usize = 2004;
const IDC_EDIT_SRC: usize = 2005;
const IDC_BTN_BROWSE_SRC: usize = 2006;
const IDC_EDIT_DST: usize = 2007;
const IDC_BTN_BROWSE_DST: usize = 2008;
const IDC_BTN_SWAP: usize = 2009;
const IDC_TAB_MAIN: usize = 2010;
const IDC_EDIT_JOB_NAME: usize = 2011;
const IDC_BTN_OPEN_SRC: usize = 2012;
const IDC_BTN_OPEN_DST: usize = 2013;

// Tab 0: 基本・除外設定
const IDC_RADIO_MODE_E: usize = 2101;
const IDC_RADIO_MODE_S: usize = 2102;
const IDC_RADIO_MODE_MIR: usize = 2103;
const IDC_RADIO_MODE_MOVE: usize = 2104;
const IDC_CHK_XJ: usize = 2105;
const IDC_CHK_FFT: usize = 2106;
const IDC_CHK_Z: usize = 2107;
const IDC_COMBO_BACKUP_PRIV: usize = 2108;
const IDC_CHK_SEC: usize = 2109;
const IDC_EDIT_XD: usize = 2110;
const IDC_BTN_XD_BROWSE: usize = 2111;
const IDC_BTN_XD_GIT: usize = 2112;
const IDC_BTN_XD_NODE: usize = 2113;
const IDC_BTN_XD_TARGET: usize = 2114;
const IDC_BTN_XD_RECYCLE: usize = 2115;
const IDC_EDIT_XF: usize = 2116;
const IDC_BTN_XF_TMP: usize = 2117;
const IDC_BTN_XF_BAK: usize = 2118;
const IDC_BTN_XF_THUMBS: usize = 2119;
const IDC_CHK_XA_H: usize = 2120;
const IDC_CHK_XA_S: usize = 2121;
const IDC_CHK_MT: usize = 2122;
const IDC_EDIT_MT: usize = 2123;
const IDC_EDIT_RETRY: usize = 2124;
const IDC_EDIT_WAIT: usize = 2125;
const IDC_CHK_AUTOLOG: usize = 2126;
const IDC_CHK_NP: usize = 2127;
const IDC_CHK_VERBOSE: usize = 2128;
const IDC_EDIT_EXTRA_ARGS: usize = 2129;
const IDC_BTN_FORM_Q_ADD: usize = 2130;
const IDC_BTN_FORM_Q_UPDATE: usize = 2131;

// Tab 1: バックアップキュー
const IDC_LV_QUEUE: usize = 2201;
const IDC_BTN_Q_NEW: usize = 2202;
const IDC_BTN_Q_EDIT: usize = 2203;
const IDC_BTN_Q_DUP: usize = 2204;
const IDC_BTN_Q_UP: usize = 2205;
const IDC_BTN_Q_DOWN: usize = 2206;
const IDC_BTN_Q_DEL: usize = 2207;
const IDC_CHK_Q_CONTINUE: usize = 2208;
const IDC_BTN_Q_CLEAR: usize = 2209;
const IDC_BTN_Q_TOGGLE_ALL: usize = 2210;

// キュー右クリックメニュー
const IDM_Q_EDIT: u32 = 3101;
const IDM_Q_RUN: u32 = 3102;
const IDM_Q_DRY: u32 = 3103;
const IDM_Q_UP: u32 = 3104;
const IDM_Q_DOWN: u32 = 3105;
const IDM_Q_DEL: u32 = 3106;
const IDM_Q_CLEAR: u32 = 3107;

// Tab 2: 実行ログ
const IDC_EDIT_LOG: usize = 2301;
const IDC_BTN_LOG_CLEAR: usize = 2302;
const IDC_BTN_LOG_COPY: usize = 2303;
const IDC_BTN_LOG_OPEN_DIR: usize = 2304;

// 下部アクション領域
const IDC_EDIT_PREVIEW: usize = 2401;
const IDC_BTN_COPY_CMD: usize = 2402;
const IDC_BTN_EXPORT_BAT: usize = 2403;
const IDC_CHK_SHUTDOWN: usize = 2404;
const IDC_BTN_RUN_SINGLE_DRY: usize = 2405;
const IDC_BTN_RUN_SINGLE: usize = 2406;
const IDC_BTN_RUN_Q_DRY: usize = 2407;
const IDC_BTN_RUN_Q: usize = 2408;
const IDC_BTN_STOP: usize = 2409;

struct UiHandles {
    menus: MenuHandles,
    fonts: ThemeFonts,
    active_tab: usize,
    _tooltip: HWND,
    mode_is_destructive: Cell<bool>,
    path_info_is_warn: Cell<bool>,
    path_info_is_ok: Cell<bool>,

    // 上部
    lbl_preset: HWND,
    combo_preset: HWND,
    btn_preset_new: HWND,
    btn_preset_save: HWND,
    btn_preset_del: HWND,
    lbl_job_name: HWND,
    edit_job_name: HWND,
    grp_paths: HWND,
    lbl_src: HWND,
    edit_src: HWND,
    btn_browse_src: HWND,
    btn_open_src: HWND,
    lbl_dst: HWND,
    edit_dst: HWND,
    btn_browse_dst: HWND,
    btn_open_dst: HWND,
    btn_swap: HWND,
    lbl_path_info: HWND,
    tab_main: HWND,

    // Tab 0
    grp_mode: HWND,
    radio_e: HWND,
    radio_s: HWND,
    radio_mir: HWND,
    radio_move: HWND,
    lbl_mode_warn: HWND,
    grp_aux: HWND,
    chk_xj: HWND,
    chk_fft: HWND,
    chk_z: HWND,
    lbl_priv: HWND,
    combo_priv: HWND,
    chk_sec: HWND,
    grp_excl: HWND,
    lbl_xd: HWND,
    edit_xd: HWND,
    btn_xd_browse: HWND,
    btn_xd_git: HWND,
    btn_xd_node: HWND,
    btn_xd_target: HWND,
    btn_xd_recycle: HWND,
    lbl_xf: HWND,
    edit_xf: HWND,
    btn_xf_tmp: HWND,
    btn_xf_bak: HWND,
    btn_xf_thumbs: HWND,
    lbl_xa: HWND,
    chk_xa_h: HWND,
    chk_xa_s: HWND,
    chk_mt: HWND,
    edit_mt: HWND,
    lbl_mt_hint: HWND,
    lbl_retry: HWND,
    edit_retry: HWND,
    lbl_wait: HWND,
    edit_wait: HWND,
    chk_autolog: HWND,
    chk_np: HWND,
    chk_verbose: HWND,
    lbl_extra: HWND,
    edit_extra: HWND,
    btn_form_q_add: HWND,
    btn_form_q_update: HWND,

    // Tab 1
    lv_queue: HWND,
    btn_q_edit: HWND,
    btn_q_dup: HWND,
    btn_q_up: HWND,
    btn_q_down: HWND,
    btn_q_del: HWND,
    btn_q_clear: HWND,
    btn_q_toggle_all: HWND,
    btn_q_new: HWND,
    chk_q_continue: HWND,

    // Tab 2
    edit_log: HWND,
    btn_log_copy: HWND,
    btn_log_open_dir: HWND,
    btn_log_clear: HWND,

    // 下部
    lbl_preview: HWND,
    edit_preview: HWND,
    btn_copy_cmd: HWND,
    btn_export_bat: HWND,
    chk_shutdown: HWND,
    btn_single_dry: HWND,
    btn_single_run: HWND,
    btn_q_dry: HWND,
    btn_q_run: HWND,
    btn_stop: HWND,
    lbl_status: HWND,
    prog_bar: HWND,
}

impl UiHandles {
    fn tab0_controls(&self) -> [HWND; 43] {
        [
            self.grp_mode,
            self.radio_e,
            self.radio_s,
            self.radio_mir,
            self.radio_move,
            self.lbl_mode_warn,
            self.grp_aux,
            self.chk_xj,
            self.chk_fft,
            self.chk_z,
            self.lbl_priv,
            self.combo_priv,
            self.chk_sec,
            self.grp_excl,
            self.lbl_xd,
            self.edit_xd,
            self.btn_xd_browse,
            self.btn_xd_git,
            self.btn_xd_node,
            self.btn_xd_target,
            self.btn_xd_recycle,
            self.lbl_xf,
            self.edit_xf,
            self.btn_xf_tmp,
            self.btn_xf_bak,
            self.btn_xf_thumbs,
            self.lbl_xa,
            self.chk_xa_h,
            self.chk_xa_s,
            self.chk_mt,
            self.edit_mt,
            self.lbl_mt_hint,
            self.lbl_retry,
            self.edit_retry,
            self.lbl_wait,
            self.edit_wait,
            self.chk_autolog,
            self.chk_np,
            self.chk_verbose,
            self.lbl_extra,
            self.edit_extra,
            self.btn_form_q_add,
            self.btn_form_q_update,
        ]
    }

    fn tab1_controls(&self) -> [HWND; 10] {
        [
            self.lv_queue,
            self.btn_q_edit,
            self.btn_q_dup,
            self.btn_q_up,
            self.btn_q_down,
            self.btn_q_del,
            self.btn_q_clear,
            self.btn_q_toggle_all,
            self.btn_q_new,
            self.chk_q_continue,
        ]
    }

    fn tab2_controls(&self) -> [HWND; 4] {
        [
            self.edit_log,
            self.btn_log_copy,
            self.btn_log_open_dir,
            self.btn_log_clear,
        ]
    }

    fn is_tab_interior_bg(&self, ctrl: HWND) -> bool {
        [
            self.grp_mode,
            self.radio_e,
            self.radio_s,
            self.radio_mir,
            self.radio_move,
            self.lbl_mode_warn,
            self.grp_aux,
            self.chk_xj,
            self.chk_fft,
            self.chk_z,
            self.lbl_priv,
            self.chk_sec,
            self.grp_excl,
            self.lbl_xd,
            self.btn_xd_browse,
            self.btn_xd_git,
            self.btn_xd_node,
            self.btn_xd_target,
            self.btn_xd_recycle,
            self.lbl_xf,
            self.btn_xf_tmp,
            self.btn_xf_bak,
            self.btn_xf_thumbs,
            self.lbl_xa,
            self.chk_xa_h,
            self.chk_xa_s,
            self.chk_mt,
            self.lbl_mt_hint,
            self.lbl_retry,
            self.lbl_wait,
            self.chk_autolog,
            self.chk_np,
            self.chk_verbose,
            self.lbl_extra,
            self.btn_form_q_add,
            self.btn_form_q_update,
            self.chk_q_continue,
            self.btn_q_edit,
            self.btn_q_dup,
            self.btn_q_up,
            self.btn_q_down,
            self.btn_q_del,
            self.btn_q_clear,
            self.btn_q_toggle_all,
            self.btn_q_new,
            self.btn_log_copy,
            self.btn_log_open_dir,
            self.btn_log_clear,
        ]
        .contains(&ctrl)
    }

    fn all_input_and_action_controls(&self) -> [HWND; 43] {
        [
            self.combo_preset,
            self.btn_preset_new,
            self.btn_preset_save,
            self.btn_preset_del,
            self.edit_job_name,
            self.edit_src,
            self.btn_browse_src,
            self.edit_dst,
            self.btn_browse_dst,
            self.btn_swap,
            self.radio_e,
            self.radio_s,
            self.radio_mir,
            self.radio_move,
            self.chk_xj,
            self.chk_fft,
            self.chk_z,
            self.combo_priv,
            self.chk_sec,
            self.edit_xd,
            self.btn_xd_browse,
            self.btn_xd_git,
            self.btn_xd_node,
            self.btn_xd_target,
            self.btn_xd_recycle,
            self.edit_xf,
            self.btn_xf_tmp,
            self.btn_xf_bak,
            self.btn_xf_thumbs,
            self.chk_xa_h,
            self.chk_xa_s,
            self.chk_mt,
            self.edit_mt,
            self.edit_retry,
            self.edit_wait,
            self.chk_autolog,
            self.chk_np,
            self.chk_verbose,
            self.edit_extra,
            self.btn_form_q_add,
            self.btn_form_q_update,
            self.lv_queue,
            self.btn_q_new,
        ]
    }
}

fn create_ctrl(
    class: &str,
    text: &str,
    style: DWORD,
    ex_style: DWORD,
    id: usize,
    parent: HWND,
    h_inst: HINSTANCE,
) -> HWND {
    let cls_w = to_wide(class);
    let txt_w = to_wide(text);
    unsafe {
        CreateWindowExW(
            ex_style,
            cls_w.as_ptr(),
            txt_w.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | style,
            0,
            0,
            10,
            10,
            parent,
            id as HMENU,
            h_inst,
            null_mut(),
        )
    }
}

fn is_checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) as WPARAM == BST_CHECKED }
}

fn set_checked(hwnd: HWND, val: bool) {
    unsafe {
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            if val { BST_CHECKED } else { BST_UNCHECKED },
            0,
        );
    }
}

fn set_tab_visibility(h: &UiHandles, tab: usize) {
    let show0 = if tab == 0 { SW_SHOW } else { SW_HIDE };
    let show1 = if tab == 1 { SW_SHOW } else { SW_HIDE };
    let show2 = if tab == 2 { SW_SHOW } else { SW_HIDE };
    unsafe {
        for c in h.tab0_controls() {
            ShowWindow(c, show0);
        }
        for c in h.tab1_controls() {
            ShowWindow(c, show1);
        }
        for c in h.tab2_controls() {
            ShowWindow(c, show2);
        }
    }
}

fn apply_fonts(h: &UiHandles) {
    let ui_f = h.fonts.ui_font.0 as WPARAM;
    let mono_f = h.fonts.mono_font.0 as WPARAM;
    let bold_f = h.fonts.bold_font.0 as WPARAM;

    let all_ui = [
        h.lbl_preset,
        h.combo_preset,
        h.btn_preset_new,
        h.btn_preset_save,
        h.btn_preset_del,
        h.lbl_job_name,
        h.edit_job_name,
        h.lbl_src,
        h.edit_src,
        h.btn_browse_src,
        h.btn_open_src,
        h.lbl_dst,
        h.edit_dst,
        h.btn_browse_dst,
        h.btn_open_dst,
        h.btn_swap,
        h.lbl_path_info,
        h.tab_main,
        h.radio_e,
        h.radio_s,
        h.radio_mir,
        h.radio_move,
        h.chk_xj,
        h.chk_fft,
        h.chk_z,
        h.lbl_priv,
        h.combo_priv,
        h.chk_sec,
        h.lbl_xd,
        h.edit_xd,
        h.btn_xd_browse,
        h.btn_xd_git,
        h.btn_xd_node,
        h.btn_xd_target,
        h.btn_xd_recycle,
        h.lbl_xf,
        h.edit_xf,
        h.btn_xf_tmp,
        h.btn_xf_bak,
        h.btn_xf_thumbs,
        h.lbl_xa,
        h.chk_xa_h,
        h.chk_xa_s,
        h.chk_mt,
        h.edit_mt,
        h.lbl_mt_hint,
        h.lbl_retry,
        h.edit_retry,
        h.lbl_wait,
        h.edit_wait,
        h.chk_autolog,
        h.chk_np,
        h.chk_verbose,
        h.lbl_extra,
        h.edit_extra,
        h.btn_form_q_add,
        h.btn_form_q_update,
        h.lv_queue,
        h.btn_q_edit,
        h.btn_q_dup,
        h.btn_q_up,
        h.btn_q_down,
        h.btn_q_del,
        h.btn_q_clear,
        h.btn_q_toggle_all,
        h.btn_q_new,
        h.chk_q_continue,
        h.btn_log_copy,
        h.btn_log_open_dir,
        h.btn_log_clear,
        h.lbl_preview,
        h.btn_copy_cmd,
        h.btn_export_bat,
        h.chk_shutdown,
        h.btn_single_dry,
        h.btn_single_run,
        h.btn_q_dry,
        h.btn_q_run,
        h.btn_stop,
        h.lbl_status,
    ];
    let bold_ctrls = [
        h.grp_paths,
        h.grp_mode,
        h.grp_aux,
        h.grp_excl,
        h.lbl_mode_warn,
    ];
    unsafe {
        for c in all_ui {
            SendMessageW(c, WM_SETFONT, ui_f, 1);
        }
        for c in bold_ctrls {
            SendMessageW(c, WM_SETFONT, bold_f, 1);
        }
        SendMessageW(h.edit_preview, WM_SETFONT, mono_f, 1);
        SendMessageW(h.edit_log, WM_SETFONT, mono_f, 1);
    }
}

fn layout_controls(hwnd: HWND, h: &UiHandles) {
    let mut rc = RECT::default();
    unsafe {
        GetClientRect(hwnd, &raw mut rc);
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let s = |v: i32| scale_dpi(v, dpi);
    let mv = |ctrl: HWND, x: i32, y: i32, w: i32, ht: i32| unsafe {
        MoveWindow(ctrl, x, y, w.max(10), ht.max(10), TRUE);
    };

    let cw = rc.right - rc.left;
    let ch = rc.bottom - rc.top;
    let pad = s(10);
    let inner_w = cw - pad * 2;

    // 1. プリセット管理・ジョブ名行
    let row_h = s(25);
    let y_preset = s(8);
    let lbl_p_cap_w = s(80);
    mv(h.lbl_preset, pad, y_preset + s(4), lbl_p_cap_w, s(18));
    let btn_new_w = s(86);
    let btn_save_w = s(98);
    let btn_del_w = s(70);
    let lbl_j_w = s(74);
    let fixed_preset_w = lbl_p_cap_w + btn_new_w + btn_save_w + btn_del_w + lbl_j_w + s(32);
    let flex_w = (inner_w - fixed_preset_w).max(s(280));
    let edit_job_w = ((flex_w * 42) / 100).max(s(130));
    let preset_combo_w = (flex_w - edit_job_w).max(s(140));
    let mut x = pad + lbl_p_cap_w + s(2);
    mv(h.combo_preset, x, y_preset, preset_combo_w, s(200));
    x += preset_combo_w + s(6);
    mv(h.btn_preset_new, x, y_preset, btn_new_w, row_h);
    x += btn_new_w + s(4);
    mv(h.btn_preset_save, x, y_preset, btn_save_w, row_h);
    x += btn_save_w + s(4);
    mv(h.btn_preset_del, x, y_preset, btn_del_w, row_h);
    x += btn_del_w + s(10);
    mv(h.lbl_job_name, x, y_preset + s(4), lbl_j_w, s(18));
    x += lbl_j_w + s(2);
    mv(
        h.edit_job_name,
        x,
        y_preset,
        (pad + inner_w - x).max(s(110)),
        row_h,
    );

    // 2. パス指定グループ
    let y_paths = s(39);
    let h_paths = s(98);
    mv(h.grp_paths, pad, y_paths, inner_w, h_paths);
    let px = pad + s(10);
    let py1 = y_paths + s(20);
    let py2 = y_paths + s(49);
    let py3 = y_paths + s(77);
    let lbl_p_w = s(82);
    let br_w = s(78);
    let open_w = s(70);
    let swap_w = s(92);
    let edit_p_w = (inner_w - s(20) - lbl_p_w - br_w - open_w - swap_w - s(16)).max(s(180));
    mv(h.lbl_src, px, py1 + s(4), lbl_p_w, s(18));
    mv(
        h.edit_src,
        px + lbl_p_w,
        py1,
        edit_p_w + swap_w + s(4),
        row_h,
    );
    mv(
        h.btn_browse_src,
        px + lbl_p_w + edit_p_w + swap_w + s(8),
        py1,
        br_w,
        row_h,
    );
    mv(
        h.btn_open_src,
        px + lbl_p_w + edit_p_w + swap_w + br_w + s(12),
        py1,
        open_w,
        row_h,
    );
    mv(h.lbl_dst, px, py2 + s(4), lbl_p_w, s(18));
    mv(h.edit_dst, px + lbl_p_w, py2, edit_p_w, row_h);
    mv(
        h.btn_browse_dst,
        px + lbl_p_w + edit_p_w + s(4),
        py2,
        br_w,
        row_h,
    );
    mv(
        h.btn_open_dst,
        px + lbl_p_w + edit_p_w + br_w + s(8),
        py2,
        open_w,
        row_h,
    );
    mv(
        h.btn_swap,
        px + lbl_p_w + edit_p_w + br_w + open_w + s(12),
        py2,
        swap_w,
        row_h,
    );
    mv(
        h.lbl_path_info,
        px + lbl_p_w,
        py3,
        (inner_w - s(20) - lbl_p_w).max(s(200)),
        s(18),
    );

    // 3. 下部固定領域サイズ予約
    let bottom_h = s(138);
    let y_bottom = (ch - bottom_h - s(6)).max(y_paths + h_paths + s(270));
    let y_tab = y_paths + h_paths + s(6);
    let h_tab = (y_bottom - y_tab - s(6)).max(s(230));
    mv(h.tab_main, pad, y_tab, inner_w, h_tab);

    // Tab 内側矩形を計算
    let mut tab_rc = RECT {
        left: pad,
        top: y_tab,
        right: pad + inner_w,
        bottom: y_tab + h_tab,
    };
    unsafe {
        SendMessageW(h.tab_main, TCM_ADJUSTRECT, 0, (&raw mut tab_rc) as LPARAM);
        SetWindowPos(h.tab_main, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE);
    }
    let tx = tab_rc.left + s(6);
    let ty = tab_rc.top + s(4);
    let tw = (tab_rc.right - tab_rc.left - s(12)).max(s(400));
    let th = (tab_rc.bottom - tab_rc.top - s(8)).max(s(180));

    // Tab 0 レイアウト（左右2分割）
    let left_w = (tw * 42) / 100;
    let right_x = tx + left_w + s(8);
    let right_w = tw - left_w - s(8);

    let mode_h = (th * 54) / 100;
    mv(h.grp_mode, tx, ty, left_w, mode_h);
    let lx = tx + s(10);
    let lw = left_w - s(20);
    mv(h.radio_e, lx, ty + s(20), lw, s(20));
    mv(h.radio_s, lx, ty + s(42), lw, s(20));
    mv(h.radio_mir, lx, ty + s(64), lw, s(20));
    mv(h.radio_move, lx, ty + s(86), lw, s(20));
    mv(
        h.lbl_mode_warn,
        lx,
        ty + s(110),
        lw,
        (mode_h - s(116)).max(s(36)),
    );

    let aux_y = ty + mode_h + s(6);
    let aux_h = (th - mode_h - s(6)).max(s(112));
    mv(h.grp_aux, tx, aux_y, left_w, aux_h);
    mv(h.chk_xj, lx, aux_y + s(18), lw, s(20));
    mv(h.chk_fft, lx, aux_y + s(40), lw / 2, s(20));
    mv(h.chk_z, lx + lw / 2, aux_y + s(40), lw / 2, s(20));
    mv(h.lbl_priv, lx, aux_y + s(66), s(84), s(18));
    mv(h.combo_priv, lx + s(86), aux_y + s(62), lw - s(86), s(120));
    mv(h.chk_sec, lx, aux_y + s(90), lw, s(20));

    // Tab 0 右側: 除外・パフォーマンス・ログ
    mv(h.grp_excl, right_x, ty, right_w, th);
    let rx = right_x + s(10);
    let rw = right_w - s(20);
    let mut ry = ty + s(18);
    mv(h.lbl_xd, rx, ry + s(4), s(118), s(18));
    let xd_btn_w = s(72);
    mv(
        h.edit_xd,
        rx + s(120),
        ry,
        (rw - s(124) - xd_btn_w).max(s(100)),
        row_h,
    );
    mv(h.btn_xd_browse, rx + rw - xd_btn_w, ry, xd_btn_w, row_h);
    ry += s(27);
    mv(h.btn_xd_git, rx, ry, s(64), s(23));
    mv(h.btn_xd_node, rx + s(68), ry, s(114), s(23));
    mv(h.btn_xd_target, rx + s(186), ry, s(74), s(23));
    mv(h.btn_xd_recycle, rx + s(264), ry, s(120), s(23));
    ry += s(27);

    mv(h.lbl_xf, rx, ry + s(4), s(118), s(18));
    mv(h.edit_xf, rx + s(120), ry, (rw - s(120)).max(s(120)), row_h);
    ry += s(27);
    mv(h.btn_xf_tmp, rx, ry, s(88), s(23));
    mv(h.btn_xf_bak, rx + s(92), ry, s(88), s(23));
    mv(h.btn_xf_thumbs, rx + s(184), ry, s(112), s(23));
    ry += s(27);

    mv(h.lbl_xa, rx, ry + s(2), s(100), s(18));
    mv(h.chk_xa_h, rx + s(102), ry, s(122), s(20));
    mv(h.chk_xa_s, rx + s(228), ry, s(128), s(20));
    ry += s(24);

    mv(h.chk_mt, rx, ry + s(2), s(130), s(20));
    mv(h.edit_mt, rx + s(132), ry, s(42), row_h);
    mv(h.lbl_retry, rx + s(182), ry + s(4), s(74), s(18));
    mv(h.edit_retry, rx + s(258), ry, s(42), row_h);
    mv(h.lbl_wait, rx + s(308), ry + s(4), s(64), s(18));
    mv(h.edit_wait, rx + s(374), ry, s(42), row_h);
    ry += s(26);
    mv(h.lbl_mt_hint, rx, ry, rw, s(18));
    ry += s(20);

    mv(h.chk_autolog, rx, ry, s(156), s(20));
    mv(h.chk_np, rx + s(160), ry, s(126), s(20));
    mv(h.chk_verbose, rx + s(290), ry, s(126), s(20));
    ry += s(24);

    mv(h.lbl_extra, rx, ry + s(4), s(132), s(18));
    mv(
        h.edit_extra,
        rx + s(134),
        ry,
        (rw - s(134)).max(s(120)),
        row_h,
    );
    ry += s(28);
    let fq_add_w = s(150);
    let fq_upd_w = s(216);
    mv(h.btn_form_q_add, rx, ry, fq_add_w, s(25));
    mv(
        h.btn_form_q_update,
        rx + fq_add_w + s(8),
        ry,
        fq_upd_w,
        s(25),
    );

    // Tab 1 レイアウト: キューListView + 下部2行操作バー
    let q_new_w = s(196);
    let lv_h = (th - s(64)).max(s(120));
    mv(h.lv_queue, tx, ty, tw, lv_h);
    resize_queue_columns(h.lv_queue, tw, dpi);
    let q_row1_y = ty + lv_h + s(6);
    mv(
        h.chk_q_continue,
        tx,
        q_row1_y + s(3),
        (tw - q_new_w - s(8)).max(s(200)),
        s(20),
    );
    mv(h.btn_q_new, tx + tw - q_new_w, q_row1_y, q_new_w, s(25));

    let q_row2_y = q_row1_y + s(29);
    let mut qx = tx;
    for (btn, w) in [
        (h.btn_q_edit, s(130)),
        (h.btn_q_dup, s(76)),
        (h.btn_q_up, s(82)),
        (h.btn_q_down, s(82)),
        (h.btn_q_del, s(76)),
        (h.btn_q_clear, s(88)),
        (h.btn_q_toggle_all, s(124)),
    ] {
        mv(btn, qx, q_row2_y, w, s(25));
        qx += w + s(6);
    }

    // Tab 2 レイアウト: ログビューア + コピー/フォルダ/クリアボタン
    mv(h.edit_log, tx, ty, tw, (th - s(34)).max(s(120)));
    let log_btn_y = ty + th - s(27);
    let log_clear_w = s(108);
    let log_open_w = s(152);
    let log_copy_w = s(132);
    mv(
        h.btn_log_copy,
        tx + tw - log_clear_w - log_open_w - log_copy_w - s(12),
        log_btn_y,
        log_copy_w,
        s(25),
    );
    mv(
        h.btn_log_open_dir,
        tx + tw - log_clear_w - log_open_w - s(6),
        log_btn_y,
        log_open_w,
        s(25),
    );
    mv(
        h.btn_log_clear,
        tx + tw - log_clear_w,
        log_btn_y,
        log_clear_w,
        s(25),
    );

    // 4. 下部アクション領域
    let mut by = y_bottom;
    let pv_lbl_w = s(92);
    let cp_w = s(94);
    let bat_w = s(100);
    let pv_edit_w = (inner_w - pv_lbl_w - cp_w - bat_w - s(14)).max(s(180));
    mv(h.lbl_preview, pad, by + s(4), pv_lbl_w, s(18));
    mv(h.edit_preview, pad + pv_lbl_w, by, pv_edit_w, row_h);
    mv(
        h.btn_copy_cmd,
        pad + pv_lbl_w + pv_edit_w + s(6),
        by,
        cp_w,
        row_h,
    );
    mv(
        h.btn_export_bat,
        pad + pv_lbl_w + pv_edit_w + cp_w + s(12),
        by,
        bat_w,
        row_h,
    );
    by += s(30);

    mv(h.chk_shutdown, pad, by, inner_w, s(20));
    by += s(26);

    let act_h = s(30);
    let stop_w = s(92);
    let run_btn_w = ((inner_w - stop_w - s(24)) / 4).max(s(120));
    mv(h.btn_single_dry, pad, by, run_btn_w, act_h);
    mv(
        h.btn_single_run,
        pad + run_btn_w + s(6),
        by,
        run_btn_w,
        act_h,
    );
    mv(
        h.btn_q_dry,
        pad + (run_btn_w + s(6)) * 2,
        by,
        run_btn_w,
        act_h,
    );
    mv(
        h.btn_q_run,
        pad + (run_btn_w + s(6)) * 3,
        by,
        run_btn_w,
        act_h,
    );
    mv(h.btn_stop, pad + inner_w - stop_w, by, stop_w, act_h);
    by += s(36);

    let prog_w = s(220);
    mv(
        h.lbl_status,
        pad,
        by + s(2),
        (inner_w - prog_w - s(10)).max(s(240)),
        s(20),
    );
    mv(h.prog_bar, pad + inner_w - prog_w, by + s(2), prog_w, s(18));
}

fn init_queue_listview(lv: HWND, dpi: u32) {
    let explorer_w = to_wide("Explorer");
    unsafe {
        SetWindowTheme(lv, explorer_w.as_ptr(), std::ptr::null());
        SendMessageW(
            lv,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES | LVS_EX_CHECKBOXES | LVS_EX_DOUBLEBUFFER)
                as LPARAM,
        );
    }
    let cols = [
        ("有効", 48),
        ("#", 36),
        ("ジョブ名", 140),
        ("コピー元", 180),
        ("コピー先", 180),
        ("モード", 110),
        ("状態", 130),
    ];
    for (idx, (title, w)) in cols.iter().enumerate() {
        let mut w_title = to_wide(title);
        let col = LVCOLUMNW {
            mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT,
            fmt: LVCFMT_LEFT,
            cx: scale_dpi(*w, dpi),
            pszText: w_title.as_mut_ptr(),
            ..Default::default()
        };
        unsafe {
            SendMessageW(lv, LVM_INSERTCOLUMNW, idx, (&raw const col) as LPARAM);
        }
    }
}

fn resize_queue_columns(lv: HWND, total_w: i32, dpi: u32) {
    let s = |v: i32| scale_dpi(v, dpi);
    let fixed_w = s(48) + s(36) + s(140) + s(110) + s(130) + s(24);
    let path_col_w = ((total_w - fixed_w) / 2).max(s(120));
    unsafe {
        SendMessageW(lv, LVM_SETCOLUMNWIDTH, 3, path_col_w as LPARAM);
        SendMessageW(lv, LVM_SETCOLUMNWIDTH, 4, path_col_w as LPARAM);
    }
}

fn sync_presets_combo(hwnd: HWND, h: &UiHandles, state: &AppState) {
    unsafe {
        SendMessageW(h.combo_preset, CB_RESETCONTENT, 0, 0);
        for p in &state.config.profiles {
            let w = to_wide(&p.name);
            SendMessageW(h.combo_preset, CB_ADDSTRING, 0, w.as_ptr() as LPARAM);
        }
        SendMessageW(
            h.combo_preset,
            CB_SETCURSEL,
            state.config.selected_preset_index,
            0,
        );
    }
    sync_preset_run_menu(
        hwnd,
        h.menus.preset_menu,
        &state.config.profiles,
        state.is_busy(),
    );
}

fn sync_queue_listview(h: &UiHandles, state: &AppState, select_idx: Option<usize>) {
    let lv = h.lv_queue;
    unsafe {
        SendMessageW(lv, LVM_DELETEALLITEMS, 0, 0);
    }
    for (idx, item) in state.config.queue.iter().enumerate() {
        let mut empty_w = to_wide("");
        let lvi = LVITEMW {
            mask: LVIF_TEXT,
            iItem: idx as INT,
            iSubItem: 0,
            pszText: empty_w.as_mut_ptr(),
            ..Default::default()
        };
        unsafe {
            SendMessageW(lv, LVM_INSERTITEMW, 0, (&raw const lvi) as LPARAM);
        }

        let num_str = format!("{}", idx + 1);
        let status_str = item.status.display_text();
        let mode_str = item.profile.copy_mode.short_label();
        let cells = [
            num_str.as_str(),
            item.name.as_str(),
            item.profile.source.as_str(),
            item.profile.destination.as_str(),
            mode_str,
            status_str.as_str(),
        ];
        for (col_offset, text) in cells.iter().enumerate() {
            let mut txt_w = to_wide(text);
            let sub = LVITEMW {
                mask: LVIF_TEXT,
                iItem: idx as INT,
                iSubItem: (col_offset + 1) as INT,
                pszText: txt_w.as_mut_ptr(),
                ..Default::default()
            };
            unsafe {
                SendMessageW(lv, LVM_SETITEMTEXTW, idx, (&raw const sub) as LPARAM);
            }
        }

        // チェック状態（1=未チェック, 2=チェック済み）
        let state_val = if item.enabled { 2 << 12 } else { 1 << 12 };
        let chk_item = LVITEMW {
            mask: LVIF_STATE,
            stateMask: LVIS_STATEIMAGEMASK,
            state: state_val,
            ..Default::default()
        };
        unsafe {
            SendMessageW(lv, LVM_SETITEMSTATE, idx, (&raw const chk_item) as LPARAM);
        }
    }

    if let Some(sel) = select_idx
        && sel < state.config.queue.len()
    {
        let sel_item = LVITEMW {
            mask: LVIF_STATE,
            stateMask: LVIS_SELECTED | LVIS_FOCUSED,
            state: LVIS_SELECTED | LVIS_FOCUSED,
            ..Default::default()
        };
        unsafe {
            SendMessageW(lv, LVM_SETITEMSTATE, sel, (&raw const sel_item) as LPARAM);
            SendMessageW(lv, LVM_ENSUREVISIBLE, sel, 0);
        }
    }
}

fn selected_queue_index(lv: HWND) -> Option<usize> {
    let idx = unsafe { SendMessageW(lv, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED) };
    if idx < 0 { None } else { Some(idx as usize) }
}

fn sync_exclusion_chip_labels(h: &UiHandles, xd: &str, xf: &str) {
    let chips = [
        (h.btn_xd_git, ".git", true),
        (h.btn_xd_node, "node_modules", true),
        (h.btn_xd_target, "target", true),
        (h.btn_xd_recycle, "$RECYCLE.BIN", true),
        (h.btn_xf_tmp, "*.tmp", false),
        (h.btn_xf_bak, "*.bak", false),
        (h.btn_xf_thumbs, "Thumbs.db", false),
    ];
    for (btn, token, is_xd) in chips {
        let src_text = if is_xd { xd } else { xf };
        let active = has_exclusion_token(src_text, token, is_xd);
        let label = if active {
            format!("✓ {token}")
        } else {
            format!("+{token}")
        };
        if get_window_text(btn) != label {
            set_window_text(btn, &label);
        }
    }
}

fn refresh_path_info_label(h: &UiHandles, source: &str, destination: &str) {
    let (msg, is_warn) = describe_paths_status(source, destination);
    let is_ok = !is_warn && !source.trim().is_empty();
    h.path_info_is_warn.set(is_warn);
    h.path_info_is_ok.set(is_ok);
    if get_window_text(h.lbl_path_info) != msg {
        set_window_text(h.lbl_path_info, &msg);
    }
}

fn sync_preset_dirty_and_queue_edit_labels(h: &UiHandles, state: &AppState) {
    let save_lbl = if state.is_current_preset_modified() {
        "上書き保存(&M)*"
    } else {
        "上書き保存(&M)"
    };
    if get_window_text(h.btn_preset_save) != save_lbl {
        set_window_text(h.btn_preset_save, save_lbl);
    }

    let upd_lbl = match state.editing_queue_position() {
        Some(pos) => format!("編集中ジョブ #{} を更新(&J)", pos + 1),
        None => "編集中ジョブを更新(&J)".to_string(),
    };
    if get_window_text(h.btn_form_q_update) != upd_lbl {
        set_window_text(h.btn_form_q_update, &upd_lbl);
    }
}

fn set_progress_running(h: &UiHandles, state: &AppState) {
    let total = state
        .active_plan
        .as_ref()
        .map_or(1, |p| p.jobs.len().max(1));
    let pos = if total <= 1 {
        35
    } else {
        ((state.current_job_idx * 100 + 50) / total).clamp(10, 95)
    };
    let bar_state = if state.phase == RunPhase::Cancelling {
        PBST_PAUSED
    } else {
        PBST_NORMAL
    };
    unsafe {
        SendMessageW(h.prog_bar, PBM_SETSTATE, bar_state, 0);
        SendMessageW(h.prog_bar, PBM_SETPOS, pos, 0);
    }
}

fn set_progress_finished(h: &UiHandles, failed: bool, cancelled: bool) {
    let bar_state = if failed {
        PBST_ERROR
    } else if cancelled {
        PBST_PAUSED
    } else {
        PBST_NORMAL
    };
    unsafe {
        SendMessageW(h.prog_bar, PBM_SETSTATE, bar_state, 0);
        SendMessageW(h.prog_bar, PBM_SETPOS, 100, 0);
    }
}

fn sync_form_to_ui(h: &UiHandles, state: &mut AppState) {
    state.is_syncing_ui = true;
    let p = &state.config.last_form;
    set_window_text(h.edit_job_name, &p.name);
    set_window_text(h.edit_src, &p.source);
    set_window_text(h.edit_dst, &p.destination);

    set_checked(h.radio_e, p.copy_mode == CopyMode::IncludeEmpty);
    set_checked(h.radio_s, p.copy_mode == CopyMode::ExcludeEmpty);
    set_checked(h.radio_mir, p.copy_mode == CopyMode::Mirror);
    set_checked(h.radio_move, p.copy_mode == CopyMode::Move);
    update_mode_warning_label(h, p.copy_mode);

    set_checked(h.chk_xj, p.exclude_junctions);
    set_checked(h.chk_fft, p.fat_time_tolerance);
    set_checked(h.chk_z, p.restartable);
    unsafe {
        SendMessageW(h.combo_priv, CB_SETCURSEL, p.backup_mode.to_index(), 0);
    }
    set_checked(h.chk_sec, p.copy_security);

    set_window_text(h.edit_xd, &p.exclude_dirs);
    set_window_text(h.edit_xf, &p.exclude_files);
    set_checked(h.chk_xa_h, p.exclude_hidden);
    set_checked(h.chk_xa_s, p.exclude_system);

    set_checked(h.chk_mt, p.multithread_enabled);
    set_window_text(h.edit_mt, &p.multithread_count.to_string());
    set_window_text(h.edit_retry, &p.retry_count.to_string());
    set_window_text(h.edit_wait, &p.wait_seconds.to_string());

    set_checked(h.chk_autolog, p.auto_save_log);
    set_checked(h.chk_np, p.no_progress);
    set_checked(h.chk_verbose, p.verbose);
    set_window_text(h.edit_extra, &p.extra_args);

    set_checked(h.chk_q_continue, state.config.continue_queue_on_error);
    set_checked(h.chk_shutdown, state.shutdown_after_completion);

    set_window_text(h.edit_preview, &state.current_preview_command());
    set_window_text(h.lbl_status, &state.status_text);
    sync_exclusion_chip_labels(h, &p.exclude_dirs, &p.exclude_files);
    refresh_path_info_label(h, &p.source, &p.destination);
    sync_preset_dirty_and_queue_edit_labels(h, state);
    state.is_syncing_ui = false;
}

fn read_form_into_profile(h: &UiHandles, current: &BackupProfile) -> BackupProfile {
    let mut p = current.clone();
    p.name = get_window_text(h.edit_job_name);
    p.source = get_window_text(h.edit_src);
    p.destination = get_window_text(h.edit_dst);
    p.copy_mode = if is_checked(h.radio_s) {
        CopyMode::ExcludeEmpty
    } else if is_checked(h.radio_mir) {
        CopyMode::Mirror
    } else if is_checked(h.radio_move) {
        CopyMode::Move
    } else {
        CopyMode::IncludeEmpty
    };
    p.exclude_junctions = is_checked(h.chk_xj);
    p.fat_time_tolerance = is_checked(h.chk_fft);
    p.restartable = is_checked(h.chk_z);
    let priv_idx = unsafe { SendMessageW(h.combo_priv, CB_GETCURSEL, 0, 0) };
    p.backup_mode = BackupPrivilegeMode::from_index(priv_idx.max(0) as usize);
    p.copy_security = is_checked(h.chk_sec);

    p.exclude_dirs = get_window_text(h.edit_xd);
    p.exclude_files = get_window_text(h.edit_xf);
    p.exclude_hidden = is_checked(h.chk_xa_h);
    p.exclude_system = is_checked(h.chk_xa_s);

    p.multithread_enabled = is_checked(h.chk_mt);
    p.multithread_count = get_window_text(h.edit_mt)
        .trim()
        .parse::<u32>()
        .unwrap_or(8)
        .clamp(1, 128);
    p.retry_count = get_window_text(h.edit_retry)
        .trim()
        .parse::<u32>()
        .unwrap_or(3)
        .min(1_000_000);
    p.wait_seconds = get_window_text(h.edit_wait)
        .trim()
        .parse::<u32>()
        .unwrap_or(5)
        .min(3600);

    p.auto_save_log = is_checked(h.chk_autolog);
    p.no_progress = is_checked(h.chk_np);
    p.verbose = is_checked(h.chk_verbose);
    p.extra_args = get_window_text(h.edit_extra);
    p
}

fn update_mode_warning_label(h: &UiHandles, mode: CopyMode) {
    h.mode_is_destructive.set(mode.is_destructive());
    let text = match mode {
        CopyMode::Mirror => {
            "【警告】/MIR はコピー先にのみ存在するファイルをすべて削除します。実行前にドライラン(/L)で確認してください。"
        }
        CopyMode::Move => {
            "【警告】/MOVE はコピー完了後にコピー元ファイルを削除します。誤指定にご注意ください。"
        }
        CopyMode::IncludeEmpty => "標準差分コピー: サブフォルダを空も含めてコピーします。",
        CopyMode::ExcludeEmpty => {
            "差分コピー: 空のサブフォルダは作成せずファイルのみコピーします。"
        }
    };
    set_window_text(h.lbl_mode_warn, text);
}

fn sync_enable_states(hwnd: HWND, h: &UiHandles, state: &AppState) {
    let busy = state.is_busy();
    let q_toolbar = [
        h.btn_q_edit,
        h.btn_q_dup,
        h.btn_q_up,
        h.btn_q_down,
        h.btn_q_del,
        h.btn_q_clear,
        h.btn_q_toggle_all,
    ];
    unsafe {
        for c in h.all_input_and_action_controls() {
            EnableWindow(c, if busy { FALSE } else { TRUE });
        }
        for c in q_toolbar {
            EnableWindow(c, FALSE);
        }
        EnableWindow(h.chk_q_continue, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_copy_cmd, TRUE);
        EnableWindow(h.btn_export_bat, if busy { FALSE } else { TRUE });
        EnableWindow(h.chk_shutdown, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_log_copy, TRUE);
        EnableWindow(h.btn_log_open_dir, TRUE);
        EnableWindow(h.btn_log_clear, if busy { FALSE } else { TRUE });

        EnableWindow(
            h.btn_open_src,
            if state.config.last_form.source.trim().is_empty() {
                FALSE
            } else {
                TRUE
            },
        );
        EnableWindow(
            h.btn_open_dst,
            if state.config.last_form.destination.trim().is_empty() {
                FALSE
            } else {
                TRUE
            },
        );

        if !busy {
            let can_swap = !state.config.last_form.source.trim().is_empty()
                && !state.config.last_form.destination.trim().is_empty();
            EnableWindow(h.btn_swap, if can_swap { TRUE } else { FALSE });
            EnableWindow(
                h.edit_mt,
                if state.config.last_form.multithread_enabled {
                    TRUE
                } else {
                    FALSE
                },
            );
            EnableWindow(
                h.btn_preset_del,
                if state.config.profiles.len() > 1 {
                    TRUE
                } else {
                    FALSE
                },
            );
            EnableWindow(
                h.btn_form_q_update,
                if state.editing_queue_id.is_some() {
                    TRUE
                } else {
                    FALSE
                },
            );

            let sel = selected_queue_index(h.lv_queue);
            let q_len = state.config.queue.len();
            EnableWindow(h.btn_q_edit, if sel.is_some() { TRUE } else { FALSE });
            EnableWindow(h.btn_q_dup, if sel.is_some() { TRUE } else { FALSE });
            EnableWindow(
                h.btn_q_up,
                if sel.is_some_and(|i| i > 0) {
                    TRUE
                } else {
                    FALSE
                },
            );
            EnableWindow(
                h.btn_q_down,
                if sel.is_some_and(|i| i + 1 < q_len) {
                    TRUE
                } else {
                    FALSE
                },
            );
            EnableWindow(h.btn_q_del, if sel.is_some() { TRUE } else { FALSE });
            EnableWindow(h.btn_q_clear, if q_len > 0 { TRUE } else { FALSE });
            EnableWindow(h.btn_q_toggle_all, if q_len > 0 { TRUE } else { FALSE });
        }

        let has_q = state.has_enabled_queue_jobs();
        EnableWindow(h.btn_single_dry, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_single_run, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_q_dry, if !busy && has_q { TRUE } else { FALSE });
        EnableWindow(h.btn_q_run, if !busy && has_q { TRUE } else { FALSE });
        EnableWindow(h.btn_stop, if busy { TRUE } else { FALSE });

        let focused = GetFocus();
        if focused == 0 || IsWindowEnabled(focused) == 0 {
            let fallback = if busy {
                h.btn_stop
            } else {
                match h.active_tab {
                    1 => h.lv_queue,
                    2 => h.edit_log,
                    _ => h.combo_preset,
                }
            };
            SetFocus(fallback);
        }
    }
    sync_menu_state(hwnd, h.menus, state.is_elevated, busy);
}

fn append_log_lines(log_edit: HWND, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push_str("\r\n");
    }
    let wide = to_wide(&text);
    unsafe {
        let mut sel_start: DWORD = 0;
        let mut sel_end: DWORD = 0;
        SendMessageW(
            log_edit,
            EM_GETSEL,
            (&raw mut sel_start) as WPARAM,
            (&raw mut sel_end) as LPARAM,
        );
        let prev_len = GetWindowTextLengthW(log_edit).max(0) as u32;
        let at_end = sel_start == sel_end && sel_end >= prev_len;

        SendMessageW(log_edit, EM_SETSEL, prev_len as WPARAM, prev_len as LPARAM);
        SendMessageW(log_edit, EM_REPLACESEL, 0, wide.as_ptr() as LPARAM);

        let mut trimmed_chars: u32 = 0;
        let total_lines = SendMessageW(log_edit, EM_GETLINECOUNT, 0, 0) as i32;
        if total_lines > MAX_GUI_LOG_LINES {
            let trim_line = total_lines - MAX_GUI_LOG_LINES;
            let char_idx = SendMessageW(log_edit, EM_LINEINDEX, trim_line as WPARAM, 0);
            if char_idx > 0 {
                trimmed_chars = char_idx as u32;
                let empty_w = to_wide("");
                SendMessageW(log_edit, EM_SETSEL, 0, char_idx as LPARAM);
                SendMessageW(log_edit, EM_REPLACESEL, 0, empty_w.as_ptr() as LPARAM);
            }
        }

        if at_end {
            let new_len = GetWindowTextLengthW(log_edit);
            SendMessageW(log_edit, EM_SETSEL, new_len as WPARAM, new_len as LPARAM);
            SendMessageW(log_edit, EM_SCROLLCARET, 0, 0);
        } else {
            let restored_start = sel_start.saturating_sub(trimmed_chars);
            let restored_end = sel_end.saturating_sub(trimmed_chars);
            SendMessageW(
                log_edit,
                EM_SETSEL,
                restored_start as WPARAM,
                restored_end as LPARAM,
            );
        }
    }
}

fn switch_to_tab(h: &mut UiHandles, tab_idx: usize) {
    h.active_tab = tab_idx;
    unsafe {
        SendMessageW(h.tab_main, TCM_SETCURSEL, tab_idx, 0);
    }
    set_tab_visibility(h, tab_idx);
}

fn switch_to_tab_with_focus(h: &mut UiHandles, tab_idx: usize) {
    let clamped = tab_idx.min(2);
    switch_to_tab(h, clamped);
    let target = match clamped {
        0 => h.edit_src,
        1 => h.lv_queue,
        _ => h.edit_log,
    };
    unsafe {
        SetFocus(target);
    }
}

fn append_semicolon_token(edit: HWND, token: &str, is_xd: bool) {
    let cur = get_window_text(edit);
    let existing = if is_xd {
        parse_xd_items(&cur)
    } else {
        parse_xf_items(&cur)
    };
    if existing.iter().any(|x| x.eq_ignore_ascii_case(token)) {
        return;
    }
    let new_val = if cur.trim().is_empty() {
        token.to_string()
    } else {
        format!("{};{}", cur.trim_end_matches([';', ' ']), token)
    };
    set_window_text(edit, &new_val);
}

fn toggle_exclusion_token_in_edit(edit: HWND, token: &str, is_xd: bool) {
    let cur = get_window_text(edit);
    let (new_val, _) = toggle_exclusion_token(&cur, token, is_xd);
    set_window_text(edit, &new_val);
}

fn focus_validation_field(h: &mut UiHandles, field: FocusField) {
    let target = match field {
        FocusField::Source => h.edit_src,
        FocusField::Destination => h.edit_dst,
        FocusField::ExtraArgs => {
            switch_to_tab(h, 0);
            h.edit_extra
        }
        FocusField::QueueList => {
            switch_to_tab(h, 1);
            h.lv_queue
        }
        FocusField::BackupMode => {
            switch_to_tab(h, 0);
            h.combo_priv
        }
    };
    unsafe {
        SetFocus(target);
    }
}

fn add_tooltip(h_tooltip: HWND, parent: HWND, ctrl: HWND, text: &str) {
    if h_tooltip == 0 || ctrl == 0 {
        return;
    }
    let mut text_w = to_wide(text);
    let ti = TTTOOLINFOW {
        uFlags: TTF_IDISHWND | TTF_SUBCLASS,
        hwnd: parent,
        uId: ctrl as usize,
        lpszText: text_w.as_mut_ptr(),
        ..Default::default()
    };
    unsafe {
        SendMessageW(h_tooltip, TTM_ADDTOOLW, 0, (&raw const ti) as LPARAM);
    }
}

fn setup_cue_banners_and_tooltips(parent: HWND, h: &UiHandles) {
    set_cue_banner(h.edit_job_name, "自動命名 (例: Docs -> Backup)");
    set_cue_banner(h.edit_src, "コピー元フォルダを入力、またはドロップ");
    set_cue_banner(h.edit_dst, "コピー先フォルダを入力、またはドロップ");
    set_cue_banner(h.edit_xd, "例: .git;node_modules;Cache");
    set_cue_banner(h.edit_xf, "例: *.tmp;*.bak;Thumbs.db");
    set_cue_banner(h.edit_extra, "例: /XO /MAXAGE:30 （/REG・/LOG系は禁止）");

    let tips: &[(HWND, &str)] = &[
        (
            h.combo_preset,
            "よく使うバックアップ設定を切り替えます。変更すると「上書き保存*」で未保存を示します。",
        ),
        (
            h.edit_job_name,
            "キュー登録時のジョブ表示名です。空欄または「既定」のまま追加するとフォルダ名から自動命名します。",
        ),
        (
            h.edit_src,
            "コピー元フォルダのパスです。エクスプローラーからドラッグ＆ドロップでも指定できます。",
        ),
        (
            h.edit_dst,
            "コピー先フォルダのパスです。存在しないフォルダを指定すると実行時に自動作成されます。",
        ),
        (
            h.btn_swap,
            "コピー元とコピー先のフォルダパスを入れ替えます。",
        ),
        (
            h.radio_e,
            "空のサブフォルダも含めてコピーします（標準の安全な差分バックアップ）。",
        ),
        (
            h.radio_s,
            "ファイルが存在するサブフォルダのみコピーします（空フォルダは作成しません）。",
        ),
        (
            h.radio_mir,
            "コピー先をコピー元と完全一致させます。コピー先にのみあるファイルは削除されるため要注意です。",
        ),
        (
            h.radio_move,
            "コピー完了後にコピー元ファイル・フォルダを削除します。移動用途向けです。",
        ),
        (
            h.chk_xj,
            "ジャンクションポイントやシンボリックリンクを辿らないようにします（循環コピー防止のため推奨）。",
        ),
        (
            h.chk_fft,
            "タイムスタンプ比較に2秒の許容誤差を設け、NASやFATドライブとの不要な再コピーを防ぎます。",
        ),
        (
            h.chk_z,
            "ネットワーク断や途中中断時に途中から再開可能なモード (/Z) でコピーします。",
        ),
        (
            h.combo_priv,
            "アクセス権不足を回避するための権限オプション（/ZB または /B）を選択します。",
        ),
        (
            h.chk_sec,
            "データ・属性・タイムスタンプに加えてNTFSアクセス権 (ACL) もコピーします (/COPY:DATS)。",
        ),
        (
            h.edit_xd,
            "除外するフォルダ名・パスをセミコロン (;) 区切りで指定します。空白を含む名前もそのまま入力できます。",
        ),
        (
            h.edit_xf,
            "除外するファイル名やワイルドカード（例: *.tmp;Thumbs.db）を指定します。",
        ),
        (
            h.chk_mt,
            "複数スレッドで並列コピーし高速化します。外付HDD/NASは1〜8、SSD間は8〜16が目安です。",
        ),
        (
            h.edit_retry,
            "使用中などでコピー失敗した際のリトライ回数 (/R) です。既定は3回です。",
        ),
        (
            h.edit_wait,
            "リトライ間の待機秒数 (/W) です。既定は5秒です。",
        ),
        (
            h.edit_extra,
            "追加の robocopy 引数を指定します。/REG・/MON・/LOG 系など危険・競合引数はブロックされます。",
        ),
        (
            h.chk_shutdown,
            "正常完了時（テスト実行・中断・終了コード8以上を除く）に60秒カウントダウン後にシャットダウンします。",
        ),
        (
            h.btn_single_dry,
            "実際のファイル変更・削除を行わずにシミュレーション (/L) を実行します。",
        ),
        (
            h.btn_single_run,
            "現在編集中の設定でバックアップを直ちに実行します。",
        ),
        (
            h.btn_q_dry,
            "チェックされたキュー項目すべてに対してシミュレーション (/L) を連続実行します。",
        ),
        (
            h.btn_q_run,
            "チェックされたキュー項目を上から順に連続バックアップ実行します。",
        ),
    ];
    for &(ctrl, text) in tips {
        add_tooltip(h._tooltip, parent, ctrl, text);
    }
}

pub unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let res = catch_unwind(AssertUnwindSafe(|| unsafe {
        main_wnd_proc_inner(hwnd, msg, wparam, lparam)
    }));
    match res {
        Ok(val) => val,
        Err(_) => {
            show_error_box(
                hwnd,
                "内部エラー",
                "予期しないパニックが発生しましたが継続保護しました。",
            );
            0
        }
    }
}

unsafe fn main_wnd_proc_inner(hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let h_inst = unsafe { GetModuleHandleW(null_mut()) };
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let is_elevated = APP_STATE.with(|c| {
                c.try_borrow()
                    .ok()
                    .and_then(|g| g.as_ref().map(|s| s.is_elevated))
                    .unwrap_or(false)
            });

            let menus = build_main_menu(hwnd, is_elevated);
            let fonts = ThemeFonts::for_dpi(dpi);

            let tt_cls = to_wide("tooltips_class32");
            let h_tooltip = unsafe {
                CreateWindowExW(
                    0,
                    tt_cls.as_ptr(),
                    std::ptr::null(),
                    WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    hwnd,
                    0,
                    h_inst,
                    null_mut(),
                )
            };
            if h_tooltip != 0 {
                unsafe {
                    SendMessageW(
                        h_tooltip,
                        TTM_SETMAXTIPWIDTH,
                        0,
                        scale_dpi(360, dpi) as LPARAM,
                    );
                }
            }

            let tab_main = create_ctrl(
                "SysTabControl32",
                "",
                WS_TABSTOP,
                0,
                IDC_TAB_MAIN,
                hwnd,
                h_inst,
            );
            for (idx, label) in ["基本・除外設定", "バックアップキュー", "実行ログ"]
                .iter()
                .enumerate()
            {
                let mut w = to_wide(label);
                let item = TCITEMW {
                    mask: TCIF_TEXT,
                    pszText: w.as_mut_ptr(),
                    ..Default::default()
                };
                unsafe {
                    SendMessageW(tab_main, TCM_INSERTITEMW, idx, (&raw const item) as LPARAM);
                }
            }

            let combo_priv = create_ctrl(
                "COMBOBOX",
                "",
                CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,
                0,
                IDC_COMBO_BACKUP_PRIV,
                hwnd,
                h_inst,
            );
            for mode in [
                BackupPrivilegeMode::None,
                BackupPrivilegeMode::RestartableFallback,
                BackupPrivilegeMode::BackupOnly,
            ] {
                let w = to_wide(mode.label());
                unsafe {
                    SendMessageW(combo_priv, CB_ADDSTRING, 0, w.as_ptr() as LPARAM);
                }
            }

            let lv_queue = create_ctrl(
                "SysListView32",
                "",
                LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS | WS_BORDER | WS_TABSTOP,
                WS_EX_CLIENTEDGE,
                IDC_LV_QUEUE,
                hwnd,
                h_inst,
            );
            init_queue_listview(lv_queue, dpi);

            let edit_log = create_ctrl(
                "EDIT",
                "",
                ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL | WS_HSCROLL | WS_TABSTOP,
                WS_EX_CLIENTEDGE,
                IDC_EDIT_LOG,
                hwnd,
                h_inst,
            );
            unsafe {
                SendMessageW(edit_log, EM_SETLIMITTEXT, 1_048_576, 0);
            }

            let prog_bar = create_ctrl("msctls_progress32", "", PBS_SMOOTH, 0, 0, hwnd, h_inst);
            unsafe {
                SendMessageW(prog_bar, PBM_SETRANGE32, 0, 100);
                SendMessageW(prog_bar, PBM_SETPOS, 0, 0);
            }

            let handles = UiHandles {
                menus,
                fonts,
                active_tab: 0,
                _tooltip: h_tooltip,
                mode_is_destructive: Cell::new(false),
                path_info_is_warn: Cell::new(false),
                path_info_is_ok: Cell::new(false),
                lbl_preset: create_ctrl("STATIC", "プリセット(&P):", SS_LEFT, 0, 0, hwnd, h_inst),
                combo_preset: create_ctrl(
                    "COMBOBOX",
                    "",
                    CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,
                    0,
                    IDC_COMBO_PRESET,
                    hwnd,
                    h_inst,
                ),
                btn_preset_new: create_ctrl(
                    "BUTTON",
                    "新規保存(&N)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_PRESET_NEW,
                    hwnd,
                    h_inst,
                ),
                btn_preset_save: create_ctrl(
                    "BUTTON",
                    "上書き保存(&M)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_PRESET_SAVE,
                    hwnd,
                    h_inst,
                ),
                btn_preset_del: create_ctrl(
                    "BUTTON",
                    "削除(&X)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_PRESET_DEL,
                    hwnd,
                    h_inst,
                ),
                lbl_job_name: create_ctrl("STATIC", "ジョブ名(&J):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_job_name: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_JOB_NAME,
                    hwnd,
                    h_inst,
                ),
                grp_paths: create_ctrl("BUTTON", "フォルダ指定", BS_GROUPBOX, 0, 0, hwnd, h_inst),
                lbl_src: create_ctrl("STATIC", "コピー元(&S):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_src: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_SRC,
                    hwnd,
                    h_inst,
                ),
                btn_browse_src: create_ctrl(
                    "BUTTON",
                    "参照...(&1)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_BROWSE_SRC,
                    hwnd,
                    h_inst,
                ),
                btn_open_src: create_ctrl(
                    "BUTTON",
                    "開く(&O)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_OPEN_SRC,
                    hwnd,
                    h_inst,
                ),
                lbl_dst: create_ctrl("STATIC", "コピー先(&D):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_dst: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_DST,
                    hwnd,
                    h_inst,
                ),
                btn_browse_dst: create_ctrl(
                    "BUTTON",
                    "参照...(&2)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_BROWSE_DST,
                    hwnd,
                    h_inst,
                ),
                btn_open_dst: create_ctrl(
                    "BUTTON",
                    "開く(&K)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_OPEN_DST,
                    hwnd,
                    h_inst,
                ),
                btn_swap: create_ctrl(
                    "BUTTON",
                    "入れ替え(&W)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_SWAP,
                    hwnd,
                    h_inst,
                ),
                lbl_path_info: create_ctrl("STATIC", "", SS_LEFT | SS_NOPREFIX, 0, 0, hwnd, h_inst),
                tab_main,

                grp_mode: create_ctrl("BUTTON", "コピーモード", BS_GROUPBOX, 0, 0, hwnd, h_inst),
                radio_e: create_ctrl(
                    "BUTTON",
                    "差分コピー: 空フォルダ含む (/E)",
                    BS_AUTORADIOBUTTON | WS_GROUP | WS_TABSTOP,
                    0,
                    IDC_RADIO_MODE_E,
                    hwnd,
                    h_inst,
                ),
                radio_s: create_ctrl(
                    "BUTTON",
                    "差分コピー: 空フォルダ除外 (/S)",
                    BS_AUTORADIOBUTTON | WS_TABSTOP,
                    0,
                    IDC_RADIO_MODE_S,
                    hwnd,
                    h_inst,
                ),
                radio_mir: create_ctrl(
                    "BUTTON",
                    "ミラーリング: 先の余分を削除 (/MIR)",
                    BS_AUTORADIOBUTTON | WS_TABSTOP,
                    0,
                    IDC_RADIO_MODE_MIR,
                    hwnd,
                    h_inst,
                ),
                radio_move: create_ctrl(
                    "BUTTON",
                    "移動: コピー後に元を削除 (/MOVE)",
                    BS_AUTORADIOBUTTON | WS_TABSTOP,
                    0,
                    IDC_RADIO_MODE_MOVE,
                    hwnd,
                    h_inst,
                ),
                lbl_mode_warn: create_ctrl(
                    "STATIC",
                    "",
                    SS_LEFT | SS_NOPREFIX | WS_GROUP,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),

                grp_aux: create_ctrl(
                    "BUTTON",
                    "補助・権限オプション",
                    BS_GROUPBOX | WS_GROUP,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                chk_xj: create_ctrl(
                    "BUTTON",
                    "ジャンクション・リンク除外 (/XJ)",
                    BS_AUTOCHECKBOX | WS_GROUP | WS_TABSTOP,
                    0,
                    IDC_CHK_XJ,
                    hwnd,
                    h_inst,
                ),
                chk_fft: create_ctrl(
                    "BUTTON",
                    "NAS/FAT 2秒比較 (/FFT)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_FFT,
                    hwnd,
                    h_inst,
                ),
                chk_z: create_ctrl(
                    "BUTTON",
                    "再開可能モード (/Z)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_Z,
                    hwnd,
                    h_inst,
                ),
                lbl_priv: create_ctrl("STATIC", "権限モード:", SS_LEFT, 0, 0, hwnd, h_inst),
                combo_priv,
                chk_sec: create_ctrl(
                    "BUTTON",
                    "セキュリティ属性もコピー (/SEC)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_SEC,
                    hwnd,
                    h_inst,
                ),

                grp_excl: create_ctrl(
                    "BUTTON",
                    "除外・パフォーマンス・ログ設定",
                    BS_GROUPBOX,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                lbl_xd: create_ctrl(
                    "STATIC",
                    "除外フォルダ(/XD ;区切):",
                    SS_LEFT,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                edit_xd: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_XD,
                    hwnd,
                    h_inst,
                ),
                btn_xd_browse: create_ctrl(
                    "BUTTON",
                    "追加...",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XD_BROWSE,
                    hwnd,
                    h_inst,
                ),
                btn_xd_git: create_ctrl(
                    "BUTTON",
                    "+.git",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XD_GIT,
                    hwnd,
                    h_inst,
                ),
                btn_xd_node: create_ctrl(
                    "BUTTON",
                    "+node_modules",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XD_NODE,
                    hwnd,
                    h_inst,
                ),
                btn_xd_target: create_ctrl(
                    "BUTTON",
                    "+target",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XD_TARGET,
                    hwnd,
                    h_inst,
                ),
                btn_xd_recycle: create_ctrl(
                    "BUTTON",
                    "+$RECYCLE.BIN",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XD_RECYCLE,
                    hwnd,
                    h_inst,
                ),
                lbl_xf: create_ctrl("STATIC", "除外ファイル(/XF):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_xf: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_XF,
                    hwnd,
                    h_inst,
                ),
                btn_xf_tmp: create_ctrl(
                    "BUTTON",
                    "+*.tmp",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XF_TMP,
                    hwnd,
                    h_inst,
                ),
                btn_xf_bak: create_ctrl(
                    "BUTTON",
                    "+*.bak",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XF_BAK,
                    hwnd,
                    h_inst,
                ),
                btn_xf_thumbs: create_ctrl(
                    "BUTTON",
                    "+Thumbs.db",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_XF_THUMBS,
                    hwnd,
                    h_inst,
                ),
                lbl_xa: create_ctrl("STATIC", "除外属性(/XA):", SS_LEFT, 0, 0, hwnd, h_inst),
                chk_xa_h: create_ctrl(
                    "BUTTON",
                    "隠しファイル(H)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_XA_H,
                    hwnd,
                    h_inst,
                ),
                chk_xa_s: create_ctrl(
                    "BUTTON",
                    "システム(S)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_XA_S,
                    hwnd,
                    h_inst,
                ),
                chk_mt: create_ctrl(
                    "BUTTON",
                    "マルチスレッド(/MT):",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_MT,
                    hwnd,
                    h_inst,
                ),
                edit_mt: create_ctrl(
                    "EDIT",
                    "8",
                    ES_NUMBER | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_MT,
                    hwnd,
                    h_inst,
                ),
                lbl_mt_hint: create_ctrl(
                    "STATIC",
                    "目安: 外付HDD/NASは1〜8推奨、SSD間は8〜16推奨",
                    SS_LEFT,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                lbl_retry: create_ctrl("STATIC", "リトライ(/R):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_retry: create_ctrl(
                    "EDIT",
                    "3",
                    ES_NUMBER | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_RETRY,
                    hwnd,
                    h_inst,
                ),
                lbl_wait: create_ctrl("STATIC", "待機(/W):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_wait: create_ctrl(
                    "EDIT",
                    "5",
                    ES_NUMBER | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_WAIT,
                    hwnd,
                    h_inst,
                ),
                chk_autolog: create_ctrl(
                    "BUTTON",
                    "logs/へ自動保存",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_AUTOLOG,
                    hwnd,
                    h_inst,
                ),
                chk_np: create_ctrl(
                    "BUTTON",
                    "進捗%抑止 (/NP)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_NP,
                    hwnd,
                    h_inst,
                ),
                chk_verbose: create_ctrl(
                    "BUTTON",
                    "スキップ詳細 (/V)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_VERBOSE,
                    hwnd,
                    h_inst,
                ),
                lbl_extra: create_ctrl(
                    "STATIC",
                    "追加引数(&O /REG禁止):",
                    SS_LEFT,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                edit_extra: create_ctrl(
                    "EDIT",
                    "",
                    ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_EXTRA_ARGS,
                    hwnd,
                    h_inst,
                ),
                btn_form_q_add: create_ctrl(
                    "BUTTON",
                    "キューへ追加(&A)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_FORM_Q_ADD,
                    hwnd,
                    h_inst,
                ),
                btn_form_q_update: create_ctrl(
                    "BUTTON",
                    "編集中ジョブを更新(&J)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_FORM_Q_UPDATE,
                    hwnd,
                    h_inst,
                ),

                lv_queue,
                btn_q_edit: create_ctrl(
                    "BUTTON",
                    "フォームへ読込(&E)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_EDIT,
                    hwnd,
                    h_inst,
                ),
                btn_q_dup: create_ctrl(
                    "BUTTON",
                    "複製(&P)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_DUP,
                    hwnd,
                    h_inst,
                ),
                btn_q_up: create_ctrl(
                    "BUTTON",
                    "↑ 上へ(&U)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_UP,
                    hwnd,
                    h_inst,
                ),
                btn_q_down: create_ctrl(
                    "BUTTON",
                    "↓ 下へ(&D)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_DOWN,
                    hwnd,
                    h_inst,
                ),
                btn_q_del: create_ctrl(
                    "BUTTON",
                    "削除(&X)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_DEL,
                    hwnd,
                    h_inst,
                ),
                btn_q_clear: create_ctrl(
                    "BUTTON",
                    "全消去(&C)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_CLEAR,
                    hwnd,
                    h_inst,
                ),
                btn_q_toggle_all: create_ctrl(
                    "BUTTON",
                    "全選択/解除(&A)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_TOGGLE_ALL,
                    hwnd,
                    h_inst,
                ),
                btn_q_new: create_ctrl(
                    "BUTTON",
                    "現在の設定を追加(&Q)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_NEW,
                    hwnd,
                    h_inst,
                ),
                chk_q_continue: create_ctrl(
                    "BUTTON",
                    "途中ジョブでエラーが発生しても残りのキューを続行する",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_Q_CONTINUE,
                    hwnd,
                    h_inst,
                ),

                edit_log,
                btn_log_copy: create_ctrl(
                    "BUTTON",
                    "ログをコピー(&Y)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_LOG_COPY,
                    hwnd,
                    h_inst,
                ),
                btn_log_open_dir: create_ctrl(
                    "BUTTON",
                    "ログフォルダを開く(&O)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_LOG_OPEN_DIR,
                    hwnd,
                    h_inst,
                ),
                btn_log_clear: create_ctrl(
                    "BUTTON",
                    "ログクリア",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_LOG_CLEAR,
                    hwnd,
                    h_inst,
                ),

                lbl_preview: create_ctrl("STATIC", "コマンド(&V):", SS_LEFT, 0, 0, hwnd, h_inst),
                edit_preview: create_ctrl(
                    "EDIT",
                    "",
                    ES_READONLY | ES_AUTOHSCROLL | WS_TABSTOP,
                    WS_EX_CLIENTEDGE,
                    IDC_EDIT_PREVIEW,
                    hwnd,
                    h_inst,
                ),
                btn_copy_cmd: create_ctrl(
                    "BUTTON",
                    "コピー(&C)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_COPY_CMD,
                    hwnd,
                    h_inst,
                ),
                btn_export_bat: create_ctrl(
                    "BUTTON",
                    ".bat出力(&B)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_EXPORT_BAT,
                    hwnd,
                    h_inst,
                ),
                chk_shutdown: create_ctrl(
                    "BUTTON",
                    "バックアップ完了後にPCをシャットダウンする（今回のみ・非永続）(&Z)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
                    0,
                    IDC_CHK_SHUTDOWN,
                    hwnd,
                    h_inst,
                ),
                btn_single_dry: create_ctrl(
                    "BUTTON",
                    "単発テスト(/L)(&L)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_RUN_SINGLE_DRY,
                    hwnd,
                    h_inst,
                ),
                btn_single_run: create_ctrl(
                    "BUTTON",
                    "単発実行(&R)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_RUN_SINGLE,
                    hwnd,
                    h_inst,
                ),
                btn_q_dry: create_ctrl(
                    "BUTTON",
                    "キューテスト(/L)(&G)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_RUN_Q_DRY,
                    hwnd,
                    h_inst,
                ),
                btn_q_run: create_ctrl(
                    "BUTTON",
                    "キュー連続実行(&U)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_RUN_Q,
                    hwnd,
                    h_inst,
                ),
                btn_stop: create_ctrl(
                    "BUTTON",
                    "中止(&K)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_STOP,
                    hwnd,
                    h_inst,
                ),
                lbl_status: create_ctrl(
                    "STATIC",
                    "待機中",
                    SS_LEFT | SS_NOPREFIX,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                prog_bar,
            };

            apply_fonts(&handles);
            setup_cue_banners_and_tooltips(hwnd, &handles);
            set_tab_visibility(&handles, 0);
            layout_controls(hwnd, &handles);

            with_state_mut(|state| {
                state.is_syncing_ui = true;
                sync_presets_combo(hwnd, &handles, state);
                sync_queue_listview(&handles, state, None);
                sync_form_to_ui(&handles, state);
                sync_enable_states(hwnd, &handles, state);
                state.is_syncing_ui = false;
            });

            let initial_focus = handles.edit_src;
            UI_HANDLES.with(|cell| {
                *cell.borrow_mut() = Some(handles);
            });
            unsafe {
                DragAcceptFiles(hwnd, TRUE);
                SetFocus(initial_focus);
            }
            0
        }

        WM_GETMINMAXINFO => {
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let mmi = unsafe { &mut *(lparam as *mut MINMAXINFO) };
            mmi.ptMinTrackSize.x = scale_dpi(920, dpi);
            mmi.ptMinTrackSize.y = scale_dpi(680, dpi);
            0
        }

        WM_SIZE => {
            with_ui(|h| layout_controls(hwnd, h));
            0
        }

        WM_DPICHANGED => {
            let new_dpi = (wparam & 0xFFFF) as u32;
            with_ui_mut(|h| {
                h.fonts = ThemeFonts::for_dpi(new_dpi);
                apply_fonts(h);
                layout_controls(hwnd, h);
            });
            let suggested = unsafe { &*(lparam as *const RECT) };
            unsafe {
                SetWindowPos(
                    hwnd,
                    0,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            0
        }

        WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            let ctrl = lparam as HWND;
            let res = with_ui(|h| unsafe {
                if ctrl == h.edit_preview || ctrl == h.edit_log {
                    SetBkMode(hdc, OPAQUE);
                    SetBkColor(hdc, COLOR_CODE_BG);
                    SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));
                    return h.fonts.code_bg_brush.0;
                }
                SetBkMode(hdc, TRANSPARENT);
                let text_color = if ctrl == h.lbl_mode_warn {
                    if h.mode_is_destructive.get() {
                        COLOR_TEXT_DANGER
                    } else {
                        COLOR_TEXT_INFO
                    }
                } else if ctrl == h.lbl_path_info {
                    if h.path_info_is_warn.get() {
                        COLOR_TEXT_DANGER
                    } else if h.path_info_is_ok.get() {
                        COLOR_TEXT_OK
                    } else {
                        GetSysColor(COLOR_WINDOWTEXT)
                    }
                } else {
                    GetSysColor(COLOR_WINDOWTEXT)
                };
                SetTextColor(hdc, text_color);
                if h.is_tab_interior_bg(ctrl) {
                    SetBkColor(hdc, COLOR_TAB_BG);
                    h.fonts.tab_bg_brush.0
                } else {
                    GetSysColorBrush(COLOR_BTNFACE)
                }
            });
            res.unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) })
        }

        WM_CTLCOLORBTN => {
            let hdc = wparam as HDC;
            let ctrl = lparam as HWND;
            let brush = with_ui(|h| {
                if h.is_tab_interior_bg(ctrl) {
                    unsafe {
                        SetBkMode(hdc, TRANSPARENT);
                        SetBkColor(hdc, COLOR_TAB_BG);
                    }
                    Some(h.fonts.tab_bg_brush.0)
                } else {
                    None
                }
            })
            .flatten();
            brush.unwrap_or_else(|| unsafe { GetSysColorBrush(COLOR_BTNFACE) })
        }

        WM_CTLCOLOREDIT => {
            let hdc = wparam as HDC;
            let ctrl = lparam as HWND;
            let brush = with_ui(|h| {
                if ctrl == h.edit_preview || ctrl == h.edit_log {
                    unsafe {
                        SetBkMode(hdc, OPAQUE);
                        SetBkColor(hdc, COLOR_CODE_BG);
                        SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));
                    }
                    Some(h.fonts.code_bg_brush.0)
                } else {
                    None
                }
            })
            .flatten();
            brush.unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) })
        }

        WM_DROPFILES => {
            handle_drop_files(hwnd, wparam as HDROP);
            0
        }

        WM_NOTIFY => {
            let nm = unsafe { &*(lparam as *const NMHDR) };
            if nm.idFrom == IDC_TAB_MAIN && nm.code == TCN_SELCHANGE {
                with_ui_mut(|h| {
                    let sel = unsafe { SendMessageW(h.tab_main, TCM_GETCURSEL, 0, 0) };
                    if sel >= 0 {
                        h.active_tab = sel as usize;
                        set_tab_visibility(h, h.active_tab);
                    }
                });
                return 0;
            }
            if nm.idFrom == IDC_LV_QUEUE {
                match nm.code {
                    LVN_ITEMCHANGED => {
                        let nmlv = unsafe { &*(lparam as *const NMLISTVIEW) };
                        if (nmlv.uChanged & LVIF_STATE) != 0 && nmlv.iItem >= 0 {
                            let state_diff = nmlv.uNewState ^ nmlv.uOldState;
                            if (state_diff & LVIS_STATEIMAGEMASK) != 0 {
                                let checked = (nmlv.uNewState & LVIS_STATEIMAGEMASK) == (2 << 12);
                                let should_sync = with_state_mut(|state| {
                                    if state.is_syncing_ui {
                                        false
                                    } else {
                                        state.set_queue_item_enabled(nmlv.iItem as usize, checked);
                                        true
                                    }
                                })
                                .unwrap_or(false);
                                if should_sync {
                                    refresh_enable_states_only(hwnd);
                                }
                            } else if (state_diff & LVIS_SELECTED) != 0 {
                                let syncing = with_state(|s| s.is_syncing_ui).unwrap_or(true);
                                if !syncing {
                                    refresh_enable_states_only(hwnd);
                                }
                            }
                        }
                        return 0;
                    }
                    NM_DBLCLK => {
                        let is_busy = with_state(AppState::is_busy).unwrap_or(true);
                        let has_sel = with_ui(|h| selected_queue_index(h.lv_queue))
                            .flatten()
                            .is_some();
                        if !is_busy && has_sel {
                            handle_queue_load(hwnd);
                        }
                        return 0;
                    }
                    LVN_KEYDOWN => {
                        let is_busy = with_state(AppState::is_busy).unwrap_or(true);
                        if !is_busy {
                            let kd = unsafe { &*(lparam as *const NMLVKEYDOWN) };
                            let alt_down = unsafe { GetKeyState(VK_MENU) } < 0;
                            match kd.wVKey {
                                VK_DELETE => handle_queue_delete(hwnd),
                                VK_UP if alt_down => handle_queue_move(hwnd, -1),
                                VK_DOWN if alt_down => handle_queue_move(hwnd, 1),
                                _ => {}
                            }
                        }
                        return 0;
                    }
                    _ => {}
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_CONTEXTMENU => {
            handle_queue_context_menu(hwnd, wparam as HWND, lparam);
            0
        }

        WM_TIMER => {
            if wparam == TIMER_ID_STATUS
                && let Some(txt) = with_state(AppState::formatted_running_status)
            {
                with_ui(|h| set_window_text(h.lbl_status, &txt));
            }
            0
        }

        WM_APP_RUNNER_EVENT => {
            handle_runner_notify(hwnd);
            0
        }

        WM_APP_SWITCH_TAB => {
            with_ui_mut(|h| switch_to_tab_with_focus(h, wparam));
            0
        }

        WM_APP_CYCLE_TAB => {
            with_ui_mut(|h| {
                let next = if wparam != 0 {
                    (h.active_tab + 1) % 3
                } else {
                    (h.active_tab + 2) % 3
                };
                switch_to_tab_with_focus(h, next);
            });
            0
        }

        WM_COMMAND => {
            handle_command(hwnd, wparam);
            0
        }

        WM_CLOSE => {
            let is_busy = with_state(AppState::is_busy).unwrap_or(false);
            if is_busy {
                let ok = confirm_yes_no(
                    hwnd,
                    "終了の確認",
                    "現在バックアップを実行中です。処理を強制中止して終了しますか？",
                );
                if !ok {
                    return 0;
                }
                with_state_mut(AppState::request_cancel);
            } else {
                // 通常終了時は設定を保存（読み取り専用パス等で失敗しても終了は妨げない）
                with_state(|s| {
                    let _ = save_config(&s.paths.config_path, &s.config);
                });
            }
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }

        WM_DESTROY => {
            unsafe {
                KillTimer(hwnd, TIMER_ID_STATUS);
                PostQuitMessage(0);
            }
            0
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn refresh_enable_states_only(hwnd: HWND) {
    with_ui_and_state(|h, s| sync_enable_states(hwnd, h, s));
}

fn capture_form_and_refresh_preview(hwnd: HWND) {
    with_ui(|h| {
        with_state_mut(|state| {
            if state.is_syncing_ui {
                return;
            }
            state.config.last_form = read_form_into_profile(h, &state.config.last_form);
            state.config.continue_queue_on_error = is_checked(h.chk_q_continue);
            state.shutdown_after_completion = is_checked(h.chk_shutdown);
            update_mode_warning_label(h, state.config.last_form.copy_mode);
            set_window_text(h.edit_preview, &state.current_preview_command());
            sync_exclusion_chip_labels(
                h,
                &state.config.last_form.exclude_dirs,
                &state.config.last_form.exclude_files,
            );
            refresh_path_info_label(
                h,
                &state.config.last_form.source,
                &state.config.last_form.destination,
            );
            sync_preset_dirty_and_queue_edit_labels(h, state);
            sync_enable_states(hwnd, h, state);
        });
    });
}

fn handle_runner_notify(hwnd: HWND) {
    let Some((new_lines, finished_report, queue_updated)) =
        with_state_mut(|state| state.process_runner_events(hwnd, WM_APP_RUNNER_EVENT))
    else {
        return;
    };

    with_ui(|h| {
        append_log_lines(h.edit_log, &new_lines);
        with_state_mut(|state| {
            if queue_updated {
                state.is_syncing_ui = true;
                let sel = selected_queue_index(h.lv_queue);
                sync_queue_listview(h, state, sel);
                state.is_syncing_ui = false;
            }
            set_window_text(h.lbl_status, &state.formatted_running_status());
            if let Some(ref report) = finished_report {
                set_progress_finished(
                    h,
                    report.summary.failed_jobs > 0,
                    report.summary.summary_line.contains("中断"),
                );
                let cur_focus = unsafe { GetFocus() };
                let move_focus_to_log = cur_focus == 0 || cur_focus == h.btn_stop;
                sync_enable_states(hwnd, h, state);
                if move_focus_to_log {
                    unsafe {
                        SetFocus(h.edit_log);
                    }
                }
            } else if queue_updated || state.phase == RunPhase::Cancelling {
                set_progress_running(h, state);
            }
        });
    });

    // ここからモーダル表示は必ず AppState 借用解放後に実行する（RefCell再入保護）
    if let Some(report) = finished_report {
        unsafe {
            KillTimer(hwnd, TIMER_ID_STATUS);
        }
        if report.summary.allow_shutdown {
            // シャットダウン前に設定ファイルを確実にフラッシュ・保存する
            with_state(|state| {
                let _ = save_config(&state.paths.config_path, &state.config);
            });
            match show_shutdown_countdown_dialog(hwnd) {
                ShutdownDialogOutcome::Proceed => {
                    with_ui(|h| {
                        append_log_lines(
                            h.edit_log,
                            &["[通知] PCのシャットダウンを実行します。".to_string()],
                        );
                    });
                    if let Err(err) = execute_system_shutdown() {
                        with_ui(|h| {
                            append_log_lines(
                                h.edit_log,
                                &[format!("[エラー] シャットダウン失敗: {err}")],
                            );
                        });
                        show_error_box(hwnd, "シャットダウン失敗", &err);
                    } else {
                        unsafe {
                            PostMessageW(hwnd, WM_CLOSE, 0, 0);
                        }
                    }
                }
                ShutdownDialogOutcome::CancelledByUser => {
                    with_ui(|h| {
                        append_log_lines(
                            h.edit_log,
                            &["[通知] シャットダウンはキャンセルされました。".to_string()],
                        );
                    });
                }
                ShutdownDialogOutcome::CancelledBySleepJump => {
                    with_ui(|h| {
                        append_log_lines(
                            h.edit_log,
                            &["[通知] スリープ復帰または長時間停止を検知したため、シャットダウンを自動中止しました。".to_string()],
                        );
                    });
                }
            }
        } else if report.summary.failed_jobs > 0 {
            show_error_box(
                hwnd,
                "バックアップ異常終了",
                &format!(
                    "{}\n\n詳細ログを確認してください。",
                    report.summary.summary_line
                ),
            );
        }
    }
}

fn is_point_in_ctrl(ctrl: HWND, screen_pt: POINT) -> bool {
    let mut rc = RECT::default();
    unsafe { GetWindowRect(ctrl, &raw mut rc) != 0 && PtInRect(&raw const rc, screen_pt) != 0 }
}

fn handle_drop_files(hwnd: HWND, h_drop: HDROP) {
    let mut pt = POINT::default();
    let count = unsafe { DragQueryFileW(h_drop, u32::MAX, null_mut(), 0) };
    let mut dropped = Vec::with_capacity(count as usize);
    let mut buf = vec![0u16; 32768];
    for idx in 0..count {
        let len = unsafe { DragQueryFileW(h_drop, idx, buf.as_mut_ptr(), buf.len() as UINT) };
        if len > 0 {
            let p = String::from_utf16_lossy(&buf[..len as usize]);
            if !p.is_empty() {
                dropped.push(p);
            }
        }
    }
    unsafe {
        DragQueryPoint(h_drop, &raw mut pt);
        ClientToScreen(hwnd, &raw mut pt);
        DragFinish(h_drop);
    }

    if dropped.is_empty() || with_state(AppState::is_busy).unwrap_or(true) {
        return;
    }

    let Some((active_tab, edit_src, edit_dst, edit_xd, edit_xf)) =
        with_ui(|h| (h.active_tab, h.edit_src, h.edit_dst, h.edit_xd, h.edit_xf))
    else {
        return;
    };

    let to_dir_string = |raw: &str| -> Option<String> {
        let p = Path::new(raw);
        if p.is_dir() {
            Some(raw.to_string())
        } else {
            p.parent()
                .map(|par| par.to_string_lossy().to_string())
                .filter(|s| !s.is_empty())
        }
    };

    let mut changed = false;
    if active_tab == 0 && is_point_in_ctrl(edit_xd, pt) {
        for raw in &dropped {
            if let Some(dir) = to_dir_string(raw) {
                append_semicolon_token(edit_xd, &dir, true);
                changed = true;
            }
        }
    } else if active_tab == 0 && is_point_in_ctrl(edit_xf, pt) {
        for raw in &dropped {
            if let Some(name) = Path::new(raw)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                && !name.is_empty()
            {
                append_semicolon_token(edit_xf, &name, false);
                changed = true;
            }
        }
    } else if is_point_in_ctrl(edit_src, pt) {
        if let Some(dir) = to_dir_string(&dropped[0]) {
            set_window_text(edit_src, &dir);
            changed = true;
        }
    } else if is_point_in_ctrl(edit_dst, pt) {
        if let Some(dir) = to_dir_string(&dropped[0]) {
            set_window_text(edit_dst, &dir);
            changed = true;
        }
    } else {
        let mut dirs = Vec::new();
        for raw in &dropped {
            if let Some(dir) = to_dir_string(raw)
                && !dirs.iter().any(|d: &String| d.eq_ignore_ascii_case(&dir))
            {
                dirs.push(dir);
            }
        }
        if dirs.len() >= 2 {
            set_window_text(edit_src, &dirs[0]);
            set_window_text(edit_dst, &dirs[1]);
            changed = true;
        } else if let Some(dir) = dirs.first() {
            let target = if get_window_text(edit_src).trim().is_empty() {
                edit_src
            } else if get_window_text(edit_dst).trim().is_empty() {
                edit_dst
            } else {
                edit_src
            };
            set_window_text(target, dir);
            changed = true;
        }
    }

    if changed {
        capture_form_and_refresh_preview(hwnd);
    }
}

fn handle_command(hwnd: HWND, wparam: WPARAM) {
    let id = wparam & 0xFFFF;
    let code = ((wparam >> 16) & 0xFFFF) as WORD;

    // フォーム入力変更系
    if matches!(
        id,
        IDC_EDIT_JOB_NAME
            | IDC_EDIT_SRC
            | IDC_EDIT_DST
            | IDC_EDIT_XD
            | IDC_EDIT_XF
            | IDC_EDIT_MT
            | IDC_EDIT_RETRY
            | IDC_EDIT_WAIT
            | IDC_EDIT_EXTRA_ARGS
    ) && code == EN_CHANGE
    {
        capture_form_and_refresh_preview(hwnd);
        return;
    }

    if matches!(
        id,
        IDC_RADIO_MODE_E
            | IDC_RADIO_MODE_S
            | IDC_RADIO_MODE_MIR
            | IDC_RADIO_MODE_MOVE
            | IDC_CHK_XJ
            | IDC_CHK_FFT
            | IDC_CHK_Z
            | IDC_CHK_SEC
            | IDC_CHK_XA_H
            | IDC_CHK_XA_S
            | IDC_CHK_MT
            | IDC_CHK_AUTOLOG
            | IDC_CHK_NP
            | IDC_CHK_VERBOSE
            | IDC_CHK_Q_CONTINUE
            | IDC_CHK_SHUTDOWN
    ) && code == BN_CLICKED
    {
        capture_form_and_refresh_preview(hwnd);
        return;
    }

    if id == IDC_COMBO_BACKUP_PRIV && code == CBN_SELCHANGE {
        capture_form_and_refresh_preview(hwnd);
        return;
    }

    if id == IDC_COMBO_PRESET && code == CBN_SELCHANGE {
        with_ui(|h| {
            let sel = unsafe { SendMessageW(h.combo_preset, CB_GETCURSEL, 0, 0) };
            if sel >= 0 {
                with_state_mut(|state| {
                    state.select_preset(sel as usize);
                    sync_form_to_ui(h, state);
                    sync_enable_states(hwnd, h, state);
                });
            }
        });
        return;
    }

    let is_busy = with_state(AppState::is_busy).unwrap_or(false);

    match id {
        x if x == IDM_FILE_SAVE as usize => {
            if !is_busy {
                handle_save_config(hwnd);
            }
        }
        x if x == IDM_FILE_EXPORT_BAT as usize || x == IDC_BTN_EXPORT_BAT => {
            if !is_busy {
                handle_export_bat(hwnd);
            }
        }
        x if x == IDM_FILE_OPEN_LOGS as usize || x == IDC_BTN_LOG_OPEN_DIR => {
            handle_open_logs(hwnd);
        }
        x if x == IDM_FILE_EXIT as usize => unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        x if x == IDM_TOOL_RESTART_ADMIN as usize => {
            if !is_busy {
                handle_restart_as_admin(hwnd);
            }
        }
        x if x == IDM_HELP_EXIT_CODES as usize => show_exit_code_help(hwnd),
        x if x == IDM_HELP_ABOUT as usize => handle_about(hwnd),
        x if (IDM_PRESET_RUN_BASE as usize
            ..(IDM_PRESET_RUN_BASE + MAX_PRESET_MENU_ITEMS) as usize)
            .contains(&x) =>
        {
            handle_run_preset_by_index(hwnd, x - IDM_PRESET_RUN_BASE as usize);
        }

        IDC_BTN_PRESET_NEW => handle_preset_new(hwnd),
        IDC_BTN_PRESET_SAVE => handle_preset_overwrite(hwnd),
        IDC_BTN_PRESET_DEL => handle_preset_delete(hwnd),

        IDC_BTN_BROWSE_SRC => browse_and_set_folder(hwnd, "コピー元フォルダを選択", |h| h.edit_src),
        IDC_BTN_BROWSE_DST => browse_and_set_folder(hwnd, "コピー先フォルダを選択", |h| h.edit_dst),
        IDC_BTN_OPEN_SRC => handle_open_path_in_explorer(hwnd, |h| h.edit_src),
        IDC_BTN_OPEN_DST => handle_open_path_in_explorer(hwnd, |h| h.edit_dst),
        IDC_BTN_SWAP => {
            with_ui(|h| {
                with_state_mut(|state| {
                    state.swap_source_and_dest();
                    sync_form_to_ui(h, state);
                    sync_enable_states(hwnd, h, state);
                });
            });
        }

        IDC_BTN_XD_BROWSE => {
            let init_src = with_ui(|h| get_window_text(h.edit_src));
            if let Some(p) = pick_folder(hwnd, "除外するフォルダを選択", init_src.as_deref())
            {
                quick_add_exclusion(hwnd, &p, true);
            }
        }
        IDC_BTN_XD_GIT => quick_toggle_exclusion(hwnd, ".git", true),
        IDC_BTN_XD_NODE => quick_toggle_exclusion(hwnd, "node_modules", true),
        IDC_BTN_XD_TARGET => quick_toggle_exclusion(hwnd, "target", true),
        IDC_BTN_XD_RECYCLE => quick_toggle_exclusion(hwnd, "$RECYCLE.BIN", true),
        IDC_BTN_XF_TMP => quick_toggle_exclusion(hwnd, "*.tmp", false),
        IDC_BTN_XF_BAK => quick_toggle_exclusion(hwnd, "*.bak", false),
        IDC_BTN_XF_THUMBS => quick_toggle_exclusion(hwnd, "Thumbs.db", false),

        IDC_BTN_Q_NEW | IDC_BTN_FORM_Q_ADD => handle_queue_add(hwnd),
        IDC_BTN_FORM_Q_UPDATE => handle_queue_update(hwnd),
        IDC_BTN_Q_EDIT => handle_queue_load(hwnd),
        IDC_BTN_Q_DUP => handle_queue_duplicate(hwnd),
        IDC_BTN_Q_UP => handle_queue_move(hwnd, -1),
        IDC_BTN_Q_DOWN => handle_queue_move(hwnd, 1),
        IDC_BTN_Q_DEL => handle_queue_delete(hwnd),
        IDC_BTN_Q_CLEAR => handle_queue_clear(hwnd),
        IDC_BTN_Q_TOGGLE_ALL => handle_queue_toggle_all(hwnd),

        IDC_BTN_LOG_COPY => handle_log_copy(hwnd),
        IDC_BTN_LOG_CLEAR => {
            with_ui(|h| set_window_text(h.edit_log, ""));
            with_state_mut(AppState::clear_log_buffer);
        }

        IDC_BTN_COPY_CMD => handle_copy_command(hwnd),
        IDC_BTN_RUN_SINGLE_DRY => handle_start_execution(hwnd, false, true),
        IDC_BTN_RUN_SINGLE => handle_start_execution(hwnd, false, false),
        IDC_BTN_RUN_Q_DRY => handle_start_execution(hwnd, true, true),
        IDC_BTN_RUN_Q => handle_start_execution(hwnd, true, false),
        IDC_BTN_STOP => {
            with_state_mut(AppState::request_cancel);
        }
        _ => {}
    }
}

fn browse_and_set_folder(hwnd: HWND, title: &str, target: impl Fn(&UiHandles) -> HWND) {
    let initial = with_ui(|h| get_window_text(target(h)));
    if let Some(p) = pick_folder(hwnd, title, initial.as_deref()) {
        with_ui(|h| set_window_text(target(h), &p));
        capture_form_and_refresh_preview(hwnd);
    }
}

fn handle_open_path_in_explorer(hwnd: HWND, target: impl FnOnce(&UiHandles) -> HWND) {
    let Some(path) = with_ui(|h| get_window_text(target(h))) else {
        return;
    };
    if let Err(err) = open_folder_or_parent_in_explorer(hwnd, &path) {
        show_warning_box(hwnd, "フォルダを開く", &err);
    }
}

fn quick_add_exclusion(hwnd: HWND, token: &str, is_xd: bool) {
    with_ui(|h| {
        let edit = if is_xd { h.edit_xd } else { h.edit_xf };
        append_semicolon_token(edit, token, is_xd);
    });
    capture_form_and_refresh_preview(hwnd);
}

fn quick_toggle_exclusion(hwnd: HWND, token: &str, is_xd: bool) {
    with_ui(|h| {
        let edit = if is_xd { h.edit_xd } else { h.edit_xf };
        toggle_exclusion_token_in_edit(edit, token, is_xd);
    });
    capture_form_and_refresh_preview(hwnd);
}

fn handle_save_config(hwnd: HWND) {
    let res = with_state(|state| save_config(&state.paths.config_path, &state.config))
        .unwrap_or_else(|| Err("状態を取得できませんでした。".to_string()));
    match res {
        Ok(()) => show_info_box(hwnd, "設定保存", "現在の設定とキューを保存しました。"),
        Err(e) => show_error_box(hwnd, "保存エラー", &e),
    }
}

fn handle_preset_new(hwnd: HWND) {
    let cur_name = with_state(|s| s.config.last_form.name.clone())
        .unwrap_or_else(|| "新規プリセット".to_string());
    let Some(new_name) = prompt_preset_name(hwnd, &cur_name) else {
        return;
    };
    let mut err_msg = None;
    with_ui(|h| {
        with_state_mut(|state| {
            if let Err(e) = state.save_as_new_preset(new_name) {
                err_msg = Some(e);
            } else {
                sync_presets_combo(hwnd, h, state);
                sync_form_to_ui(h, state);
                sync_enable_states(hwnd, h, state);
            }
        });
    });
    if let Some(e) = err_msg {
        show_error_box(hwnd, "プリセット保存エラー", &e);
    }
}

fn handle_preset_overwrite(hwnd: HWND) {
    let res = with_state_mut(AppState::overwrite_current_preset)
        .unwrap_or_else(|| Err("状態を取得できませんでした。".to_string()));
    match res {
        Ok(()) => {
            with_ui_and_state(sync_preset_dirty_and_queue_edit_labels);
            show_info_box(
                hwnd,
                "上書き保存",
                "選択中のプリセットを更新・保存しました。",
            );
        }
        Err(e) => show_error_box(hwnd, "保存エラー", &e),
    }
}

fn handle_preset_delete(hwnd: HWND) {
    if !confirm_yes_no(hwnd, "プリセット削除", "選択中のプリセットを削除しますか？")
    {
        return;
    }
    let mut err_msg = None;
    with_ui(|h| {
        with_state_mut(|state| {
            if let Err(e) = state.delete_current_preset() {
                err_msg = Some(e);
            } else {
                sync_presets_combo(hwnd, h, state);
                sync_form_to_ui(h, state);
                sync_enable_states(hwnd, h, state);
            }
        });
    });
    if let Some(e) = err_msg {
        show_error_box(hwnd, "削除エラー", &e);
    }
}

fn handle_run_preset_by_index(hwnd: HWND, idx: usize) {
    let mut selected = false;
    with_ui(|h| {
        with_state_mut(|state| {
            if state.is_busy() || idx >= state.config.profiles.len() {
                return;
            }
            state.select_preset(idx);
            state.is_syncing_ui = true;
            unsafe {
                SendMessageW(h.combo_preset, CB_SETCURSEL, idx, 0);
            }
            sync_form_to_ui(h, state);
            sync_enable_states(hwnd, h, state);
            state.is_syncing_ui = false;
            selected = true;
        });
    });
    if selected {
        handle_start_execution(hwnd, false, false);
    }
}

fn handle_queue_add(hwnd: HWND) {
    with_ui_and_state_mut(|h, state| {
        let new_idx = state.add_current_to_queue();
        state.is_syncing_ui = true;
        sync_queue_listview(h, state, Some(new_idx));
        state.is_syncing_ui = false;
        sync_preset_dirty_and_queue_edit_labels(h, state);
        sync_enable_states(hwnd, h, state);
        switch_to_tab(h, 1);
        unsafe {
            SetFocus(h.lv_queue);
        }
    });
}

fn handle_queue_update(hwnd: HWND) {
    let updated = with_ui_and_state_mut(|h, state| {
        let idx = state.update_editing_queue_item()?;
        state.is_syncing_ui = true;
        sync_queue_listview(h, state, Some(idx));
        state.is_syncing_ui = false;
        sync_preset_dirty_and_queue_edit_labels(h, state);
        sync_enable_states(hwnd, h, state);
        switch_to_tab(h, 1);
        unsafe {
            SetFocus(h.lv_queue);
        }
        Some(idx)
    })
    .flatten();
    if updated.is_none() {
        show_warning_box(
            hwnd,
            "ジョブ更新",
            "更新対象のキュージョブが見つかりません。",
        );
    }
}

fn handle_queue_load(hwnd: HWND) {
    let Some(idx) = with_ui(|h| selected_queue_index(h.lv_queue)).flatten() else {
        show_warning_box(hwnd, "キュー読込", "読み込むキュー項目を選択してください。");
        return;
    };
    with_ui_and_state_mut(|h, state| {
        if state.load_queue_item_to_form(idx) {
            sync_form_to_ui(h, state);
            sync_enable_states(hwnd, h, state);
        }
        switch_to_tab(h, 0);
        unsafe {
            SetFocus(h.edit_src);
        }
    });
}

fn handle_queue_duplicate(hwnd: HWND) {
    let Some(idx) = with_ui(|h| selected_queue_index(h.lv_queue)).flatten() else {
        return;
    };
    with_ui_and_state_mut(|h, state| {
        if let Some(new_idx) = state.duplicate_queue_item(idx) {
            state.is_syncing_ui = true;
            sync_queue_listview(h, state, Some(new_idx));
            state.is_syncing_ui = false;
            sync_preset_dirty_and_queue_edit_labels(h, state);
            sync_enable_states(hwnd, h, state);
        }
    });
}

fn handle_queue_move(hwnd: HWND, delta: isize) {
    with_ui(|h| {
        let Some(idx) = selected_queue_index(h.lv_queue) else {
            return;
        };
        with_state_mut(|state| {
            if let Some(new_idx) = state.move_queue_item(idx, delta) {
                state.is_syncing_ui = true;
                sync_queue_listview(h, state, Some(new_idx));
                state.is_syncing_ui = false;
                sync_preset_dirty_and_queue_edit_labels(h, state);
                sync_enable_states(hwnd, h, state);
            }
        });
    });
}

fn handle_queue_delete(hwnd: HWND) {
    with_ui(|h| {
        let Some(idx) = selected_queue_index(h.lv_queue) else {
            return;
        };
        with_state_mut(|state| {
            if state.delete_queue_item(idx) {
                let next_sel = idx.min(state.config.queue.len().saturating_sub(1));
                state.is_syncing_ui = true;
                sync_queue_listview(h, state, Some(next_sel));
                state.is_syncing_ui = false;
                sync_preset_dirty_and_queue_edit_labels(h, state);
                sync_enable_states(hwnd, h, state);
            }
        });
    });
}

fn handle_queue_clear(hwnd: HWND) {
    if !confirm_yes_no(
        hwnd,
        "キュー全消去",
        "キュー内のすべてのジョブを削除しますか？",
    ) {
        return;
    }
    with_ui(|h| {
        with_state_mut(|state| {
            state.clear_queue();
            state.is_syncing_ui = true;
            sync_queue_listview(h, state, None);
            state.is_syncing_ui = false;
            sync_preset_dirty_and_queue_edit_labels(h, state);
            sync_enable_states(hwnd, h, state);
        });
    });
}

fn handle_queue_toggle_all(hwnd: HWND) {
    with_ui_and_state_mut(|h, state| {
        if state.config.queue.is_empty() {
            return;
        }
        state.toggle_all_queue_items();
        let sel = selected_queue_index(h.lv_queue);
        state.is_syncing_ui = true;
        sync_queue_listview(h, state, sel);
        state.is_syncing_ui = false;
        sync_enable_states(hwnd, h, state);
    });
}

fn handle_log_copy(hwnd: HWND) {
    let text = with_ui(|h| get_window_text(h.edit_log)).unwrap_or_default();
    if text.trim().is_empty() {
        show_warning_box(hwnd, "ログコピー", "コピー対象のログがありません。");
        return;
    }
    match copy_text_to_clipboard(hwnd, &text) {
        Ok(()) => show_info_box(
            hwnd,
            "ログコピー完了",
            "実行ログの内容をクリップボードへコピーしました。",
        ),
        Err(e) => show_error_box(hwnd, "コピー失敗", &e),
    }
}

fn handle_copy_command(hwnd: HWND) {
    let cmd = with_state(AppState::current_preview_command).unwrap_or_default();
    match copy_text_to_clipboard(hwnd, &cmd) {
        Ok(()) => show_info_box(
            hwnd,
            "コピー完了",
            "コマンド文字列をクリップボードへコピーしました。",
        ),
        Err(e) => show_error_box(hwnd, "コピー失敗", &e),
    }
}

fn handle_export_bat(hwnd: HWND) {
    let Some((use_queue, cfg)) =
        with_state(|state| (state.has_enabled_queue_jobs(), state.config.clone()))
    else {
        return;
    };

    let export_queue = use_queue
        && confirm_yes_no(
            hwnd,
            ".bat 出力対象の選択",
            "有効なキュー項目が存在します。\n「はい」= キュー全体を連続実行する .bat を出力\n「いいえ」= 現在の編集設定のみを出力",
        );

    let pause_at_end = confirm_yes_no(
        hwnd,
        "終了時の pause 指定",
        "バッチ実行完了後に確認待ち（pause）を入れますか？\n（タスクスケジューラ等での無人実行時は「いいえ」を推奨します）",
    );

    let bat_res = if export_queue {
        let pairs: Vec<(&str, &BackupProfile)> = cfg
            .queue
            .iter()
            .filter(|q| q.enabled)
            .map(|q| (q.name.as_str(), &q.profile))
            .collect();
        build_bat_script(&pairs, false, pause_at_end, cfg.continue_queue_on_error)
    } else {
        build_bat_script(
            &[(cfg.last_form.name.as_str(), &cfg.last_form)],
            false,
            pause_at_end,
            true,
        )
    };

    let script = match bat_res {
        Ok(s) => s,
        Err(e) => {
            show_error_box(hwnd, ".bat 生成エラー", &e.message);
            return;
        }
    };

    let Some(path) = pick_save_bat_path(hwnd, "robocopy_backup.bat") else {
        return;
    };
    match export_bat_file(&path, &script) {
        Ok(()) => show_info_box(
            hwnd,
            ".bat 出力完了",
            &format!(
                "バッチファイルを UTF-8 (chcp 65001) + CRLF 形式で保存しました:\n{}",
                path.display()
            ),
        ),
        Err(e) => show_error_box(hwnd, "保存エラー", &e),
    }
}

fn handle_open_logs(hwnd: HWND) {
    let Some(dir) = with_state(|s| s.paths.logs_dir.clone()) else {
        return;
    };
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    let op_w = to_wide("open");
    let dir_w = to_wide(&dir.to_string_lossy());
    unsafe {
        ShellExecuteW(
            hwnd,
            op_w.as_ptr(),
            dir_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOW,
        );
    }
}

fn handle_about(hwnd: HWND) {
    if let Some((cfg, logs, elev)) = with_state(|s| {
        (
            s.paths.config_path.to_string_lossy().to_string(),
            s.paths.logs_dir.to_string_lossy().to_string(),
            s.is_elevated,
        )
    }) {
        show_about_dialog(hwnd, &cfg, &logs, elev);
    }
}

fn handle_restart_as_admin(hwnd: HWND) {
    let Some((busy, elevated, cfg, cfg_path)) = with_state(|s| {
        (
            s.is_busy(),
            s.is_elevated,
            s.config.clone(),
            s.paths.config_path.clone(),
        )
    }) else {
        return;
    };
    if busy || elevated {
        return;
    }

    // マップドドライブが含まれる場合は管理者プロセス側で認識できない恐れがあるため事前警告
    let mut path_refs: Vec<&str> = vec![
        cfg.last_form.source.as_str(),
        cfg.last_form.destination.as_str(),
    ];
    for q in &cfg.queue {
        path_refs.push(q.profile.source.as_str());
        path_refs.push(q.profile.destination.as_str());
    }
    let mapped = detect_mapped_drives(&path_refs);
    if !mapped.is_empty() {
        let list: Vec<String> = mapped
            .iter()
            .map(|(d, u)| format!("  {d} -> {u}"))
            .collect();
        let cont = confirm_yes_no(
            hwnd,
            "ネットワークドライブの注意",
            &format!(
                "ネットワークドライブ（マップドドライブ）が設定に含まれています。\n\
                 UAC昇格後の管理者プロセスではドライブ文字が参照できない場合があります（UNCパス指定を推奨）:\n{}\n\n\
                 このまま管理者として再起動を続けますか？",
                list.join("\n")
            ),
        );
        if !cont {
            return;
        }
    }

    // 設定ファイルと引き継ぎ一時ファイルの両方へ安全保存を試みる
    let _ = save_config(&cfg_path, &cfg);
    let handoff = write_handoff_config(&cfg).ok();

    match restart_as_admin(hwnd, handoff.as_deref()) {
        Ok(()) => unsafe {
            DestroyWindow(hwnd);
        },
        Err(e) => {
            if let Some(hp) = handoff {
                let _ = std::fs::remove_file(hp);
            }
            show_warning_box(hwnd, "管理者再起動キャンセル", &e);
        }
    }
}

fn handle_queue_context_menu(hwnd: HWND, target: HWND, lparam: LPARAM) {
    let Some(lv) = with_ui(|h| h.lv_queue) else {
        return;
    };
    if target != lv {
        return;
    }

    let (busy, q_len) = with_state(|s| (s.is_busy(), s.config.queue.len())).unwrap_or((true, 0));
    if busy {
        return;
    }

    let raw_x = (lparam & 0xFFFF) as i16 as i32;
    let raw_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    let screen_pt = if raw_x == -1 && raw_y == -1 {
        let mut pt = if let Some(sel_idx) = selected_queue_index(lv) {
            let mut rc = RECT {
                left: LVIR_BOUNDS,
                ..Default::default()
            };
            unsafe {
                SendMessageW(lv, LVM_GETITEMRECT, sel_idx, (&raw mut rc) as LPARAM);
            }
            POINT {
                x: rc.left + 16,
                y: (rc.top + rc.bottom) / 2,
            }
        } else {
            POINT { x: 16, y: 16 }
        };
        unsafe {
            ClientToScreen(lv, &raw mut pt);
        }
        pt
    } else {
        let mut client_pt = POINT { x: raw_x, y: raw_y };
        unsafe {
            ScreenToClient(lv, &raw mut client_pt);
            let mut hti = LVHITTESTINFO {
                pt: client_pt,
                ..Default::default()
            };
            let hit = SendMessageW(lv, LVM_HITTEST, 0, (&raw mut hti) as LPARAM) as i32;
            if hit >= 0 {
                let mut lvi = LVITEMW {
                    stateMask: LVIS_SELECTED | LVIS_FOCUSED,
                    state: LVIS_SELECTED | LVIS_FOCUSED,
                    ..Default::default()
                };
                SendMessageW(lv, LVM_SETITEMSTATE, hit as usize, (&raw mut lvi) as LPARAM);
            }
        }
        POINT { x: raw_x, y: raw_y }
    };

    let sel = selected_queue_index(lv);
    let has_sel = sel.is_some();
    let can_up = sel.is_some_and(|i| i > 0);
    let can_down = sel.is_some_and(|i| i + 1 < q_len);
    let has_any = q_len > 0;

    let cmd = unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return;
        }
        let append_item = |id: u32, label: &str, enabled: bool| {
            let flags = MF_STRING | if enabled { MF_ENABLED } else { MF_GRAYED };
            let w = to_wide(label);
            AppendMenuW(menu, flags, id as usize, w.as_ptr());
        };
        append_item(IDM_Q_EDIT, "編集(&E)", has_sel);
        append_item(IDM_Q_RUN, "実行（バックアップ開始）(&R)", has_sel);
        append_item(IDM_Q_DRY, "テスト(/L)で実行(&T)", has_sel);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        append_item(IDM_Q_UP, "上へ移動(&U)", can_up);
        append_item(IDM_Q_DOWN, "下へ移動(&D)", can_down);
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        append_item(IDM_Q_DEL, "削除(&X)", has_sel);
        append_item(IDM_Q_CLEAR, "キュー全消去(&C)", has_any);

        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            screen_pt.x,
            screen_pt.y,
            0,
            hwnd,
            std::ptr::null(),
        ) as u32;
        DestroyMenu(menu);
        chosen
    };

    match cmd {
        IDM_Q_EDIT => handle_queue_load(hwnd),
        IDM_Q_RUN => handle_start_queue_item_execution(hwnd, false),
        IDM_Q_DRY => handle_start_queue_item_execution(hwnd, true),
        IDM_Q_UP => handle_queue_move(hwnd, -1),
        IDM_Q_DOWN => handle_queue_move(hwnd, 1),
        IDM_Q_DEL => handle_queue_delete(hwnd),
        IDM_Q_CLEAR => handle_queue_clear(hwnd),
        _ => {}
    }
}

fn sync_shutdown_checkbox_to_state() {
    with_ui_and_state_mut(|h, s| {
        s.shutdown_after_completion = is_checked(h.chk_shutdown);
    });
}

fn handle_start_queue_item_execution(hwnd: HWND, is_dry_run: bool) {
    sync_shutdown_checkbox_to_state();
    let Some(idx) = with_ui(|h| selected_queue_index(h.lv_queue)).flatten() else {
        return;
    };
    let Some((plan, validation)) =
        with_state(|s| s.prepare_plan_for_queue_item(idx, is_dry_run)).flatten()
    else {
        return;
    };
    execute_validated_plan(hwnd, plan, validation);
}

fn handle_start_execution(hwnd: HWND, is_queue: bool, is_dry_run: bool) {
    sync_shutdown_checkbox_to_state();
    let Some((plan, validation)) = with_state(|s| s.prepare_plan(is_queue, is_dry_run)) else {
        return;
    };
    execute_validated_plan(hwnd, plan, validation);
}

fn execute_validated_plan(
    hwnd: HWND,
    plan: crate::domain::queue::ExecutionPlan,
    validation: crate::domain::validator::ValidationResult,
) {
    if !validation.is_ok() {
        let field = validation.errors[0].0;
        let all_msgs: Vec<&str> = validation.errors.iter().map(|(_, m)| m.as_str()).collect();
        show_error_box(hwnd, "実行前安全チェックエラー", &all_msgs.join("\n\n"));
        with_ui_mut(|h| focus_validation_field(h, field));
        return;
    }

    if (!validation.destructive_summaries.is_empty() || plan.shutdown_after_completion)
        && !confirm_pre_execution(
            hwnd,
            &validation.destructive_summaries,
            &validation.command_previews,
            &validation.warnings,
            plan.shutdown_after_completion,
        )
    {
        return;
    }

    let affects_queue = plan.is_queue_mode || plan.jobs.iter().any(|j| j.queue_index.is_some());
    let init_lines = with_state_mut(|state| state.begin_execution(plan, hwnd, WM_APP_RUNNER_EVENT))
        .unwrap_or_default();

    with_ui_and_state_mut(|h, state| {
        set_window_text(h.edit_log, "");
        append_log_lines(h.edit_log, &init_lines);
        switch_to_tab(h, 2);
        if affects_queue {
            state.is_syncing_ui = true;
            let sel = selected_queue_index(h.lv_queue);
            sync_queue_listview(h, state, sel);
            state.is_syncing_ui = false;
        }
        set_window_text(h.lbl_status, &state.formatted_running_status());
        set_progress_running(h, state);
        sync_enable_states(hwnd, h, state);
        // 実行ボタン無効化によるフォーカス喪失（NULL化）を防ぎ、ログ欄へ安全にフォーカスを移す。
        unsafe {
            SetFocus(h.edit_log);
        }
    });

    unsafe {
        SetTimer(hwnd, TIMER_ID_STATUS, 500, 0);
    }
}
