use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::null_mut;

use crate::app::{AppState, FinishedExecutionReport};
use crate::domain::command::{build_bat_script, parse_xd_items, parse_xf_items};
use crate::domain::profile::{BackupPrivilegeMode, BackupProfile, CopyMode};
use crate::domain::validator::FocusField;
use crate::infra::storage::{export_bat_file, save_config, write_handoff_config};
use crate::infra::system::{detect_mapped_drives, execute_system_shutdown, restart_as_admin};
use crate::infra::win32_ffi::*;

use super::dialogs::{
    confirm_pre_execution, confirm_yes_no, pick_folder, pick_save_bat_path, prompt_preset_name,
    show_about_dialog, show_error_box, show_exit_code_help, show_info_box,
    show_shutdown_countdown_dialog, show_warning_box,
};
use super::menu::*;
use super::theme::ThemeFonts;

pub const WM_APP_RUNNER_EVENT: UINT = WM_APP + 1;
const TIMER_ID_STATUS: usize = 1;
const MAX_GUI_LOG_LINES: i32 = 2000;

thread_local! {
    pub static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    static UI_HANDLES: RefCell<Option<UiHandles>> = const { RefCell::new(None) };
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

// Tab 1: バックアップキュー
const IDC_LV_QUEUE: usize = 2201;
const IDC_BTN_Q_ADD: usize = 2202;
const IDC_BTN_Q_LOAD: usize = 2203;
const IDC_BTN_Q_UP: usize = 2204;
const IDC_BTN_Q_DOWN: usize = 2205;
const IDC_BTN_Q_DEL: usize = 2206;
const IDC_BTN_Q_CLEAR: usize = 2207;
const IDC_CHK_Q_CONTINUE: usize = 2208;

// Tab 2: 実行ログ
const IDC_EDIT_LOG: usize = 2301;
const IDC_BTN_LOG_CLEAR: usize = 2302;

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
    menu: HMENU,
    fonts: ThemeFonts,
    active_tab: usize,

    // 上部
    lbl_preset: HWND,
    combo_preset: HWND,
    btn_preset_new: HWND,
    btn_preset_save: HWND,
    btn_preset_del: HWND,
    grp_paths: HWND,
    lbl_src: HWND,
    edit_src: HWND,
    btn_browse_src: HWND,
    lbl_dst: HWND,
    edit_dst: HWND,
    btn_browse_dst: HWND,
    btn_swap: HWND,
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

    // Tab 1
    lv_queue: HWND,
    btn_q_add: HWND,
    btn_q_load: HWND,
    btn_q_up: HWND,
    btn_q_down: HWND,
    btn_q_del: HWND,
    btn_q_clear: HWND,
    chk_q_continue: HWND,

    // Tab 2
    edit_log: HWND,
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
}

impl UiHandles {
    fn tab0_controls(&self) -> [HWND; 39] {
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
        ]
    }

    fn tab0_extra_controls(&self) -> [HWND; 2] {
        [self.lbl_extra, self.edit_extra]
    }

    fn tab1_controls(&self) -> [HWND; 8] {
        [
            self.lv_queue,
            self.btn_q_add,
            self.btn_q_load,
            self.btn_q_up,
            self.btn_q_down,
            self.btn_q_del,
            self.btn_q_clear,
            self.chk_q_continue,
        ]
    }

    fn tab2_controls(&self) -> [HWND; 2] {
        [self.edit_log, self.btn_log_clear]
    }

    fn all_input_and_action_controls(&self) -> [HWND; 45] {
        [
            self.combo_preset,
            self.btn_preset_new,
            self.btn_preset_save,
            self.btn_preset_del,
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
            self.lv_queue,
            self.btn_q_add,
            self.btn_q_load,
            self.btn_q_up,
            self.btn_q_down,
            self.btn_q_del,
            self.btn_q_clear,
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
        for c in h.tab0_extra_controls() {
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
        h.grp_paths,
        h.lbl_src,
        h.edit_src,
        h.btn_browse_src,
        h.lbl_dst,
        h.edit_dst,
        h.btn_browse_dst,
        h.btn_swap,
        h.tab_main,
        h.grp_mode,
        h.radio_e,
        h.radio_s,
        h.radio_mir,
        h.radio_move,
        h.grp_aux,
        h.chk_xj,
        h.chk_fft,
        h.chk_z,
        h.lbl_priv,
        h.combo_priv,
        h.chk_sec,
        h.grp_excl,
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
        h.lv_queue,
        h.btn_q_add,
        h.btn_q_load,
        h.btn_q_up,
        h.btn_q_down,
        h.btn_q_del,
        h.btn_q_clear,
        h.chk_q_continue,
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
    unsafe {
        for c in all_ui {
            SendMessageW(c, WM_SETFONT, ui_f, 1);
        }
        SendMessageW(h.lbl_mode_warn, WM_SETFONT, bold_f, 1);
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

    // 1. プリセット管理行
    let row_h = s(24);
    let y_preset = s(8);
    mv(h.lbl_preset, pad, y_preset + s(4), s(82), s(18));
    let btn_w = s(86);
    let preset_combo_w = (inner_w - s(86) - btn_w * 3 - s(18)).max(s(140));
    mv(
        h.combo_preset,
        pad + s(84),
        y_preset,
        preset_combo_w,
        s(200),
    );
    let bx = pad + s(84) + preset_combo_w + s(6);
    mv(h.btn_preset_new, bx, y_preset, btn_w, row_h);
    mv(h.btn_preset_save, bx + btn_w + s(4), y_preset, btn_w, row_h);
    mv(
        h.btn_preset_del,
        bx + (btn_w + s(4)) * 2,
        y_preset,
        btn_w,
        row_h,
    );

    // 2. パス指定グループ
    let y_paths = s(38);
    let h_paths = s(74);
    mv(h.grp_paths, pad, y_paths, inner_w, h_paths);
    let px = pad + s(10);
    let py1 = y_paths + s(18);
    let py2 = y_paths + s(45);
    let lbl_p_w = s(82);
    let br_w = s(82);
    let swap_w = s(96);
    let edit_p_w = (inner_w - s(20) - lbl_p_w - br_w - swap_w - s(14)).max(s(180));
    mv(h.lbl_src, px, py1 + s(4), lbl_p_w, s(18));
    mv(
        h.edit_src,
        px + lbl_p_w,
        py1,
        edit_p_w + swap_w + s(6),
        row_h,
    );
    mv(
        h.btn_browse_src,
        px + lbl_p_w + edit_p_w + swap_w + s(12),
        py1,
        br_w,
        row_h,
    );
    mv(h.lbl_dst, px, py2 + s(4), lbl_p_w, s(18));
    mv(h.edit_dst, px + lbl_p_w, py2, edit_p_w, row_h);
    mv(
        h.btn_browse_dst,
        px + lbl_p_w + edit_p_w + s(6),
        py2,
        br_w,
        row_h,
    );
    mv(
        h.btn_swap,
        px + lbl_p_w + edit_p_w + br_w + s(12),
        py2,
        swap_w,
        row_h,
    );

    // 3. 下部固定領域サイズ予約
    let bottom_h = s(134);
    let y_bottom = (ch - bottom_h - s(6)).max(y_paths + h_paths + s(240));
    let y_tab = y_paths + h_paths + s(6);
    let h_tab = (y_bottom - y_tab - s(6)).max(s(220));
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
        SetWindowPos(h.tab_main, HWND_BOTTOM, 0, 0, 0, 0, 0x0001 | 0x0002); // SWP_NOSIZE | SWP_NOMOVE
    }
    let tx = tab_rc.left + s(6);
    let ty = tab_rc.top + s(4);
    let tw = (tab_rc.right - tab_rc.left - s(12)).max(s(400));
    let th = (tab_rc.bottom - tab_rc.top - s(8)).max(s(180));

    // Tab 0 レイアウト（左右2分割）
    let left_w = (tw * 42) / 100;
    let right_x = tx + left_w + s(8);
    let right_w = tw - left_w - s(8);

    let mode_h = (th * 55) / 100;
    mv(h.grp_mode, tx, ty, left_w, mode_h);
    let lx = tx + s(10);
    let lw = left_w - s(20);
    mv(h.radio_e, lx, ty + s(18), lw, s(20));
    mv(h.radio_s, lx, ty + s(39), lw, s(20));
    mv(h.radio_mir, lx, ty + s(60), lw, s(20));
    mv(h.radio_move, lx, ty + s(81), lw, s(20));
    mv(
        h.lbl_mode_warn,
        lx,
        ty + s(104),
        lw,
        (mode_h - s(110)).max(s(36)),
    );

    let aux_y = ty + mode_h + s(6);
    let aux_h = (th - mode_h - s(6)).max(s(108));
    mv(h.grp_aux, tx, aux_y, left_w, aux_h);
    mv(h.chk_xj, lx, aux_y + s(16), lw, s(20));
    mv(h.chk_fft, lx, aux_y + s(37), lw / 2, s(20));
    mv(h.chk_z, lx + lw / 2, aux_y + s(37), lw / 2, s(20));
    mv(h.lbl_priv, lx, aux_y + s(62), s(84), s(18));
    mv(h.combo_priv, lx + s(86), aux_y + s(58), lw - s(86), s(120));
    mv(h.chk_sec, lx, aux_y + s(86), lw, s(20));

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
    let qbw = s(84);
    mv(h.btn_xd_git, rx, ry, s(54), s(22));
    mv(h.btn_xd_node, rx + s(58), ry, s(96), s(22));
    mv(h.btn_xd_target, rx + s(158), ry, s(62), s(22));
    mv(h.btn_xd_recycle, rx + s(224), ry, s(106), s(22));
    ry += s(26);

    mv(h.lbl_xf, rx, ry + s(4), s(118), s(18));
    mv(h.edit_xf, rx + s(120), ry, (rw - s(120)).max(s(120)), row_h);
    ry += s(27);
    mv(h.btn_xf_tmp, rx, ry, qbw, s(22));
    mv(h.btn_xf_bak, rx + qbw + s(4), ry, qbw, s(22));
    mv(h.btn_xf_thumbs, rx + (qbw + s(4)) * 2, ry, s(94), s(22));
    ry += s(26);

    mv(h.lbl_xa, rx, ry + s(2), s(100), s(18));
    mv(h.chk_xa_h, rx + s(102), ry, s(118), s(20));
    mv(h.chk_xa_s, rx + s(224), ry, s(128), s(20));
    ry += s(24);

    mv(h.chk_mt, rx, ry + s(2), s(126), s(20));
    mv(h.edit_mt, rx + s(128), ry, s(42), row_h);
    mv(h.lbl_retry, rx + s(178), ry + s(4), s(74), s(18));
    mv(h.edit_retry, rx + s(254), ry, s(42), row_h);
    mv(h.lbl_wait, rx + s(304), ry + s(4), s(64), s(18));
    mv(h.edit_wait, rx + s(370), ry, s(42), row_h);
    ry += s(26);
    mv(h.lbl_mt_hint, rx, ry, rw, s(18));
    ry += s(20);

    mv(h.chk_autolog, rx, ry, s(156), s(20));
    mv(h.chk_np, rx + s(160), ry, s(124), s(20));
    mv(h.chk_verbose, rx + s(288), ry, s(120), s(20));
    ry += s(24);

    mv(h.lbl_extra, rx, ry + s(4), s(132), s(18));
    mv(
        h.edit_extra,
        rx + s(134),
        ry,
        (rw - s(134)).max(s(120)),
        row_h,
    );

    // Tab 1 レイアウト: キューListView + 操作ボタン
    let q_btn_w = s(172);
    let lv_w = (tw - q_btn_w - s(8)).max(s(300));
    let lv_h = (th - s(28)).max(s(120));
    mv(h.lv_queue, tx, ty, lv_w, lv_h);
    let qbx = tx + lv_w + s(8);
    let qbh = s(26);
    mv(h.btn_q_add, qbx, ty, q_btn_w, qbh);
    mv(h.btn_q_load, qbx, ty + s(32), q_btn_w, qbh);
    mv(h.btn_q_up, qbx, ty + s(68), q_btn_w, qbh);
    mv(h.btn_q_down, qbx, ty + s(98), q_btn_w, qbh);
    mv(h.btn_q_del, qbx, ty + s(134), q_btn_w, qbh);
    mv(h.btn_q_clear, qbx, ty + s(164), q_btn_w, qbh);
    mv(h.chk_q_continue, tx, ty + lv_h + s(6), tw, s(20));

    // Tab 2 レイアウト: ログビューア
    let log_clear_w = s(110);
    mv(h.edit_log, tx, ty, tw, (th - s(32)).max(s(120)));
    mv(
        h.btn_log_clear,
        tx + tw - log_clear_w,
        ty + th - s(26),
        log_clear_w,
        s(24),
    );

    // 4. 下部アクション領域
    let mut by = y_bottom;
    let pv_lbl_w = s(92);
    let cp_w = s(92);
    let bat_w = s(96);
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

    mv(h.lbl_status, pad, by, inner_w, s(22));
}

fn init_queue_listview(lv: HWND, dpi: u32) {
    unsafe {
        SendMessageW(
            lv,
            LVM_SETEXTENDEDLISTVIEWSTYLE,
            0,
            (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES | LVS_EX_CHECKBOXES) as LPARAM,
        );
    }
    let cols = [
        ("有効", 48),
        ("#", 34),
        ("ジョブ名", 110),
        ("コピー元", 160),
        ("コピー先", 160),
        ("モード", 96),
        ("状態", 120),
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

fn sync_presets_combo(h: &UiHandles, state: &AppState) {
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

fn sync_form_to_ui(h: &UiHandles, state: &mut AppState) {
    state.is_syncing_ui = true;
    let p = &state.config.last_form;
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
    state.is_syncing_ui = false;
}

fn read_form_into_profile(h: &UiHandles, current: &BackupProfile) -> BackupProfile {
    let mut p = current.clone();
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
    unsafe {
        for c in h.all_input_and_action_controls() {
            EnableWindow(c, if busy { FALSE } else { TRUE });
        }
        EnableWindow(h.chk_q_continue, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_copy_cmd, TRUE);
        EnableWindow(h.btn_export_bat, if busy { FALSE } else { TRUE });
        EnableWindow(h.chk_shutdown, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_log_clear, if busy { FALSE } else { TRUE });

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
        }

        let has_q = state.has_enabled_queue_jobs();
        EnableWindow(h.btn_single_dry, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_single_run, if busy { FALSE } else { TRUE });
        EnableWindow(h.btn_q_dry, if !busy && has_q { TRUE } else { FALSE });
        EnableWindow(h.btn_q_run, if !busy && has_q { TRUE } else { FALSE });
        EnableWindow(h.btn_stop, if busy { TRUE } else { FALSE });
    }
    sync_menu_state(hwnd, h.menu, state.is_elevated, busy);
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
        let len = GetWindowTextLengthW(log_edit);
        SendMessageW(log_edit, EM_SETSEL, len as WPARAM, len as LPARAM);
        SendMessageW(log_edit, EM_REPLACESEL, 0, wide.as_ptr() as LPARAM);

        let total_lines = SendMessageW(log_edit, EM_GETLINECOUNT, 0, 0) as i32;
        if total_lines > MAX_GUI_LOG_LINES {
            let trim_line = total_lines - MAX_GUI_LOG_LINES;
            let char_idx = SendMessageW(log_edit, EM_LINEINDEX, trim_line as WPARAM, 0);
            if char_idx > 0 {
                let empty_w = to_wide("");
                SendMessageW(log_edit, EM_SETSEL, 0, char_idx as LPARAM);
                SendMessageW(log_edit, EM_REPLACESEL, 0, empty_w.as_ptr() as LPARAM);
                let new_len = GetWindowTextLengthW(log_edit);
                SendMessageW(log_edit, EM_SETSEL, new_len as WPARAM, new_len as LPARAM);
            }
        }
        SendMessageW(log_edit, EM_SCROLLCARET, 0, 0);
    }
}

fn switch_to_tab(h: &UiHandles, tab_idx: usize) {
    unsafe {
        SendMessageW(h.tab_main, TCM_SETCURSEL, tab_idx, 0);
    }
    set_tab_visibility(h, tab_idx);
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

fn focus_validation_field(h: &UiHandles, field: FocusField) {
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

            let menu = build_main_menu(hwnd, is_elevated);
            let fonts = ThemeFonts::for_dpi(dpi);

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

            let handles = UiHandles {
                menu,
                fonts,
                active_tab: 0,
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
                btn_swap: create_ctrl(
                    "BUTTON",
                    "入れ替え(&W)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_SWAP,
                    hwnd,
                    h_inst,
                ),
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
                lbl_mode_warn: create_ctrl("STATIC", "", SS_LEFT | SS_NOPREFIX, 0, 0, hwnd, h_inst),

                grp_aux: create_ctrl(
                    "BUTTON",
                    "補助・権限オプション",
                    BS_GROUPBOX,
                    0,
                    0,
                    hwnd,
                    h_inst,
                ),
                chk_xj: create_ctrl(
                    "BUTTON",
                    "ジャンクション・リンク除外 (/XJ)",
                    BS_AUTOCHECKBOX | WS_TABSTOP,
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

                lv_queue,
                btn_q_add: create_ctrl(
                    "BUTTON",
                    "現在設定を追加(&Q)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_ADD,
                    hwnd,
                    h_inst,
                ),
                btn_q_load: create_ctrl(
                    "BUTTON",
                    "編集欄へ読込(&I)",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_LOAD,
                    hwnd,
                    h_inst,
                ),
                btn_q_up: create_ctrl(
                    "BUTTON",
                    "上へ",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_UP,
                    hwnd,
                    h_inst,
                ),
                btn_q_down: create_ctrl(
                    "BUTTON",
                    "下へ",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_DOWN,
                    hwnd,
                    h_inst,
                ),
                btn_q_del: create_ctrl(
                    "BUTTON",
                    "選択削除",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_DEL,
                    hwnd,
                    h_inst,
                ),
                btn_q_clear: create_ctrl(
                    "BUTTON",
                    "キュー全消去",
                    BS_PUSHBUTTON | WS_TABSTOP,
                    0,
                    IDC_BTN_Q_CLEAR,
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
            };

            apply_fonts(&handles);
            set_tab_visibility(&handles, 0);
            layout_controls(hwnd, &handles);

            APP_STATE.with(|cell| {
                if let Ok(mut guard) = cell.try_borrow_mut()
                    && let Some(state) = guard.as_mut()
                {
                    state.is_syncing_ui = true;
                    sync_presets_combo(&handles, state);
                    sync_queue_listview(&handles, state, None);
                    sync_form_to_ui(&handles, state);
                    sync_enable_states(hwnd, &handles, state);
                    state.is_syncing_ui = false;
                }
            });

            UI_HANDLES.with(|cell| {
                *cell.borrow_mut() = Some(handles);
            });
            0
        }

        WM_GETMINMAXINFO => {
            let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
            let mmi = unsafe { &mut *(lparam as *mut MINMAXINFO) };
            mmi.ptMinTrackSize.x = scale_dpi(880, dpi);
            mmi.ptMinTrackSize.y = scale_dpi(640, dpi);
            0
        }

        WM_SIZE => {
            UI_HANDLES.with(|cell| {
                if let Ok(guard) = cell.try_borrow()
                    && let Some(h) = guard.as_ref()
                {
                    layout_controls(hwnd, h);
                }
            });
            0
        }

        WM_DPICHANGED => {
            let new_dpi = (wparam & 0xFFFF) as u32;
            UI_HANDLES.with(|cell| {
                if let Ok(mut guard) = cell.try_borrow_mut()
                    && let Some(h) = guard.as_mut()
                {
                    h.fonts = ThemeFonts::for_dpi(new_dpi);
                    apply_fonts(h);
                    layout_controls(hwnd, h);
                }
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
            let is_warn = UI_HANDLES.with(|cell| {
                cell.try_borrow()
                    .ok()
                    .and_then(|g| g.as_ref().map(|h| h.lbl_mode_warn == ctrl))
                    .unwrap_or(false)
            });
            unsafe {
                SetBkMode(hdc, TRANSPARENT);
                if is_warn {
                    // 警告テキスト用（濃い赤茶系）
                    SetTextColor(hdc, 0x0000_00A0);
                } else {
                    SetTextColor(hdc, GetSysColor(COLOR_WINDOWTEXT));
                }
                GetSysColorBrush(COLOR_BTNFACE)
            }
        }

        WM_NOTIFY => {
            let nm = unsafe { &*(lparam as *const NMHDR) };
            if nm.idFrom == IDC_TAB_MAIN && nm.code == TCN_SELCHANGE {
                UI_HANDLES.with(|cell| {
                    if let Ok(mut guard) = cell.try_borrow_mut()
                        && let Some(h) = guard.as_mut()
                    {
                        let sel = unsafe { SendMessageW(h.tab_main, TCM_GETCURSEL, 0, 0) };
                        if sel >= 0 {
                            h.active_tab = sel as usize;
                            set_tab_visibility(h, h.active_tab);
                        }
                    }
                });
                return 0;
            }
            if nm.idFrom == IDC_LV_QUEUE && nm.code == LVN_ITEMCHANGED {
                let nmlv = unsafe { &*(lparam as *const NMLISTVIEW) };
                if (nmlv.uChanged & LVIF_STATE) != 0
                    && ((nmlv.uNewState ^ nmlv.uOldState) & LVIS_STATEIMAGEMASK) != 0
                    && nmlv.iItem >= 0
                {
                    let checked = (nmlv.uNewState & LVIS_STATEIMAGEMASK) == (2 << 12);
                    let mut should_sync = false;
                    APP_STATE.with(|cell| {
                        if let Ok(mut guard) = cell.try_borrow_mut()
                            && let Some(state) = guard.as_mut()
                            && !state.is_syncing_ui
                        {
                            state.set_queue_item_enabled(nmlv.iItem as usize, checked);
                            should_sync = true;
                        }
                    });
                    if should_sync {
                        refresh_enable_states_only(hwnd);
                    }
                }
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_TIMER => {
            if wparam == TIMER_ID_STATUS {
                let status_str = APP_STATE.with(|cell| {
                    cell.try_borrow()
                        .ok()
                        .and_then(|g| g.as_ref().map(AppState::formatted_running_status))
                });
                if let Some(txt) = status_str {
                    UI_HANDLES.with(|cell| {
                        if let Ok(g) = cell.try_borrow()
                            && let Some(h) = g.as_ref()
                        {
                            set_window_text(h.lbl_status, &txt);
                        }
                    });
                }
            }
            0
        }

        WM_APP_RUNNER_EVENT => {
            handle_runner_notify(hwnd);
            0
        }

        WM_COMMAND => {
            handle_command(hwnd, wparam);
            0
        }

        WM_CLOSE => {
            let is_busy = APP_STATE.with(|c| {
                c.try_borrow()
                    .ok()
                    .and_then(|g| g.as_ref().map(AppState::is_busy))
                    .unwrap_or(false)
            });
            if is_busy {
                let ok = confirm_yes_no(
                    hwnd,
                    "終了の確認",
                    "現在バックアップを実行中です。処理を強制中止して終了しますか？",
                );
                if !ok {
                    return 0;
                }
                APP_STATE.with(|c| {
                    if let Ok(mut g) = c.try_borrow_mut()
                        && let Some(s) = g.as_mut()
                    {
                        s.request_cancel();
                    }
                });
            } else {
                // 通常終了時は設定を保存（読み取り専用パス等で失敗しても終了は妨げない）
                APP_STATE.with(|c| {
                    if let Ok(g) = c.try_borrow()
                        && let Some(s) = g.as_ref()
                    {
                        let _ = save_config(&s.paths.config_path, &s.config);
                    }
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
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(sg) = st.try_borrow()
                    && let Some(s) = sg.as_ref()
                {
                    sync_enable_states(hwnd, h, s);
                }
            });
        }
    });
}

fn capture_form_and_refresh_preview(hwnd: HWND) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    if state.is_syncing_ui {
                        return;
                    }
                    let updated = read_form_into_profile(h, &state.config.last_form);
                    state.update_form_profile(updated);
                    state.config.continue_queue_on_error = is_checked(h.chk_q_continue);
                    state.shutdown_after_completion = is_checked(h.chk_shutdown);
                    update_mode_warning_label(h, state.config.last_form.copy_mode);
                    set_window_text(h.edit_preview, &state.current_preview_command());
                    sync_enable_states(hwnd, h, state);
                }
            });
        }
    });
}

fn handle_runner_notify(hwnd: HWND) {
    let mut new_lines = Vec::new();
    let mut finished_report: Option<FinishedExecutionReport> = None;
    let mut queue_updated = false;

    APP_STATE.with(|cell| {
        if let Ok(mut guard) = cell.try_borrow_mut()
            && let Some(state) = guard.as_mut()
        {
            let (lines, rep, q_upd) = state.process_runner_events(hwnd, WM_APP_RUNNER_EVENT);
            new_lines = lines;
            finished_report = rep;
            queue_updated = q_upd;
        }
    });

    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            append_log_lines(h.edit_log, &new_lines);
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    if queue_updated {
                        state.is_syncing_ui = true;
                        sync_queue_listview(h, state, None);
                        state.is_syncing_ui = false;
                    }
                    set_window_text(h.lbl_status, &state.formatted_running_status());
                    if finished_report.is_some() {
                        sync_enable_states(hwnd, h, state);
                    }
                }
            });
        }
    });

    // ここからモーダル表示は必ず AppState 借用解放後に実行する（RefCell再入保護）
    if let Some(report) = finished_report {
        unsafe {
            KillTimer(hwnd, TIMER_ID_STATUS);
        }
        if report.summary.allow_shutdown {
            // シャットダウン前に設定ファイルを確実にフラッシュ・保存する
            APP_STATE.with(|st| {
                if let Ok(sg) = st.try_borrow()
                    && let Some(state) = sg.as_ref()
                {
                    let _ = save_config(&state.paths.config_path, &state.config);
                }
            });
            if show_shutdown_countdown_dialog(hwnd) {
                if let Err(err) = execute_system_shutdown() {
                    show_error_box(hwnd, "シャットダウン失敗", &err);
                }
            } else {
                UI_HANDLES.with(|uh| {
                    if let Ok(hg) = uh.try_borrow()
                        && let Some(h) = hg.as_ref()
                    {
                        append_log_lines(
                            h.edit_log,
                            &["[通知] シャットダウンはキャンセルされました。".to_string()],
                        );
                    }
                });
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

fn handle_command(hwnd: HWND, wparam: WPARAM) {
    let id = wparam & 0xFFFF;
    let code = ((wparam >> 16) & 0xFFFF) as WORD;

    // フォーム入力変更系
    if matches!(
        id,
        IDC_EDIT_SRC
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
        UI_HANDLES.with(|uh| {
            if let Ok(hg) = uh.try_borrow()
                && let Some(h) = hg.as_ref()
            {
                let sel = unsafe { SendMessageW(h.combo_preset, CB_GETCURSEL, 0, 0) };
                if sel >= 0 {
                    APP_STATE.with(|st| {
                        if let Ok(mut sg) = st.try_borrow_mut()
                            && let Some(state) = sg.as_mut()
                        {
                            state.select_preset(sel as usize);
                            sync_form_to_ui(h, state);
                            sync_enable_states(hwnd, h, state);
                        }
                    });
                }
            }
        });
        return;
    }

    match id {
        x if x == IDM_FILE_SAVE as usize => handle_save_config(hwnd),
        x if x == IDM_FILE_EXPORT_BAT as usize || x == IDC_BTN_EXPORT_BAT => {
            handle_export_bat(hwnd)
        }
        x if x == IDM_FILE_OPEN_LOGS as usize => handle_open_logs(hwnd),
        x if x == IDM_FILE_EXIT as usize => unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        },
        x if x == IDM_TOOL_RESTART_ADMIN as usize => handle_restart_as_admin(hwnd),
        x if x == IDM_HELP_EXIT_CODES as usize => show_exit_code_help(hwnd),
        x if x == IDM_HELP_ABOUT as usize => handle_about(hwnd),

        IDC_BTN_PRESET_NEW => handle_preset_new(hwnd),
        IDC_BTN_PRESET_SAVE => handle_preset_overwrite(hwnd),
        IDC_BTN_PRESET_DEL => handle_preset_delete(hwnd),

        IDC_BTN_BROWSE_SRC => {
            if let Some(p) = pick_folder(hwnd, "コピー元フォルダを選択") {
                UI_HANDLES.with(|uh| {
                    if let Ok(hg) = uh.try_borrow()
                        && let Some(h) = hg.as_ref()
                    {
                        set_window_text(h.edit_src, &p);
                    }
                });
                capture_form_and_refresh_preview(hwnd);
            }
        }
        IDC_BTN_BROWSE_DST => {
            if let Some(p) = pick_folder(hwnd, "コピー先フォルダを選択") {
                UI_HANDLES.with(|uh| {
                    if let Ok(hg) = uh.try_borrow()
                        && let Some(h) = hg.as_ref()
                    {
                        set_window_text(h.edit_dst, &p);
                    }
                });
                capture_form_and_refresh_preview(hwnd);
            }
        }
        IDC_BTN_SWAP => {
            UI_HANDLES.with(|uh| {
                if let Ok(hg) = uh.try_borrow()
                    && let Some(h) = hg.as_ref()
                {
                    APP_STATE.with(|st| {
                        if let Ok(mut sg) = st.try_borrow_mut()
                            && let Some(state) = sg.as_mut()
                        {
                            state.swap_source_and_dest();
                            sync_form_to_ui(h, state);
                            sync_enable_states(hwnd, h, state);
                        }
                    });
                }
            });
        }

        IDC_BTN_XD_BROWSE => {
            if let Some(p) = pick_folder(hwnd, "除外するフォルダを選択") {
                UI_HANDLES.with(|uh| {
                    if let Ok(hg) = uh.try_borrow()
                        && let Some(h) = hg.as_ref()
                    {
                        append_semicolon_token(h.edit_xd, &p, true);
                    }
                });
                capture_form_and_refresh_preview(hwnd);
            }
        }
        IDC_BTN_XD_GIT => quick_add_xd(hwnd, ".git"),
        IDC_BTN_XD_NODE => quick_add_xd(hwnd, "node_modules"),
        IDC_BTN_XD_TARGET => quick_add_xd(hwnd, "target"),
        IDC_BTN_XD_RECYCLE => quick_add_xd(hwnd, "$RECYCLE.BIN"),
        IDC_BTN_XF_TMP => quick_add_xf(hwnd, "*.tmp"),
        IDC_BTN_XF_BAK => quick_add_xf(hwnd, "*.bak"),
        IDC_BTN_XF_THUMBS => quick_add_xf(hwnd, "Thumbs.db"),

        IDC_BTN_Q_ADD => handle_queue_add(hwnd),
        IDC_BTN_Q_LOAD => handle_queue_load(hwnd),
        IDC_BTN_Q_UP => handle_queue_move(hwnd, -1),
        IDC_BTN_Q_DOWN => handle_queue_move(hwnd, 1),
        IDC_BTN_Q_DEL => handle_queue_delete(hwnd),
        IDC_BTN_Q_CLEAR => handle_queue_clear(hwnd),
        IDC_BTN_LOG_CLEAR => {
            UI_HANDLES.with(|uh| {
                if let Ok(hg) = uh.try_borrow()
                    && let Some(h) = hg.as_ref()
                {
                    set_window_text(h.edit_log, "");
                }
            });
        }

        IDC_BTN_COPY_CMD => handle_copy_command(hwnd),
        IDC_BTN_RUN_SINGLE_DRY => handle_start_execution(hwnd, false, true),
        IDC_BTN_RUN_SINGLE => handle_start_execution(hwnd, false, false),
        IDC_BTN_RUN_Q_DRY => handle_start_execution(hwnd, true, true),
        IDC_BTN_RUN_Q => handle_start_execution(hwnd, true, false),
        IDC_BTN_STOP => {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    state.request_cancel();
                }
            });
        }
        _ => {}
    }
}

fn quick_add_xd(hwnd: HWND, token: &str) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            append_semicolon_token(h.edit_xd, token, true);
        }
    });
    capture_form_and_refresh_preview(hwnd);
}

fn quick_add_xf(hwnd: HWND, token: &str) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            append_semicolon_token(h.edit_xf, token, false);
        }
    });
    capture_form_and_refresh_preview(hwnd);
}

fn handle_save_config(hwnd: HWND) {
    let res = APP_STATE.with(|st| {
        if let Ok(sg) = st.try_borrow()
            && let Some(state) = sg.as_ref()
        {
            return save_config(&state.paths.config_path, &state.config);
        }
        Err("状態を取得できませんでした。".to_string())
    });
    match res {
        Ok(()) => show_info_box(hwnd, "設定保存", "現在の設定とキューを保存しました。"),
        Err(e) => show_error_box(hwnd, "保存エラー", &e),
    }
}

fn handle_preset_new(hwnd: HWND) {
    let cur_name = APP_STATE.with(|st| {
        st.try_borrow()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.config.last_form.name.clone()))
            .unwrap_or_else(|| "新規プリセット".to_string())
    });
    let Some(new_name) = prompt_preset_name(hwnd, &cur_name) else {
        return;
    };
    let mut err_msg = None;
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    if let Err(e) = state.save_as_new_preset(new_name) {
                        err_msg = Some(e);
                    } else {
                        sync_presets_combo(h, state);
                        sync_enable_states(hwnd, h, state);
                    }
                }
            });
        }
    });
    if let Some(e) = err_msg {
        show_error_box(hwnd, "プリセット保存エラー", &e);
    }
}

fn handle_preset_overwrite(hwnd: HWND) {
    let res = APP_STATE.with(|st| {
        if let Ok(mut sg) = st.try_borrow_mut()
            && let Some(state) = sg.as_mut()
        {
            return state.overwrite_current_preset();
        }
        Err("状態を取得できませんでした。".to_string())
    });
    match res {
        Ok(()) => show_info_box(
            hwnd,
            "上書き保存",
            "選択中のプリセットを更新・保存しました。",
        ),
        Err(e) => show_error_box(hwnd, "保存エラー", &e),
    }
}

fn handle_preset_delete(hwnd: HWND) {
    if !confirm_yes_no(hwnd, "プリセット削除", "選択中のプリセットを削除しますか？")
    {
        return;
    }
    let mut err_msg = None;
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    if let Err(e) = state.delete_current_preset() {
                        err_msg = Some(e);
                    } else {
                        sync_presets_combo(h, state);
                        sync_form_to_ui(h, state);
                        sync_enable_states(hwnd, h, state);
                    }
                }
            });
        }
    });
    if let Some(e) = err_msg {
        show_error_box(hwnd, "削除エラー", &e);
    }
}

fn handle_queue_add(hwnd: HWND) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    let new_idx = state.add_current_to_queue();
                    state.is_syncing_ui = true;
                    sync_queue_listview(h, state, Some(new_idx));
                    state.is_syncing_ui = false;
                    sync_enable_states(hwnd, h, state);
                }
            });
            switch_to_tab(h, 1);
        }
    });
}

fn handle_queue_load(hwnd: HWND) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            let Some(idx) = selected_queue_index(h.lv_queue) else {
                show_warning_box(hwnd, "キュー読込", "読み込むキュー項目を選択してください。");
                return;
            };
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                    && state.load_queue_item_to_form(idx)
                {
                    sync_form_to_ui(h, state);
                    sync_enable_states(hwnd, h, state);
                }
            });
            switch_to_tab(h, 0);
        }
    });
}

fn handle_queue_move(hwnd: HWND, delta: isize) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            let Some(idx) = selected_queue_index(h.lv_queue) else {
                return;
            };
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                    && let Some(new_idx) = state.move_queue_item(idx, delta)
                {
                    state.is_syncing_ui = true;
                    sync_queue_listview(h, state, Some(new_idx));
                    state.is_syncing_ui = false;
                    sync_enable_states(hwnd, h, state);
                }
            });
        }
    });
}

fn handle_queue_delete(hwnd: HWND) {
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            let Some(idx) = selected_queue_index(h.lv_queue) else {
                return;
            };
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                    && state.delete_queue_item(idx)
                {
                    let next_sel = idx.min(state.config.queue.len().saturating_sub(1));
                    state.is_syncing_ui = true;
                    sync_queue_listview(h, state, Some(next_sel));
                    state.is_syncing_ui = false;
                    sync_enable_states(hwnd, h, state);
                }
            });
        }
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
    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    state.clear_queue();
                    state.is_syncing_ui = true;
                    sync_queue_listview(h, state, None);
                    state.is_syncing_ui = false;
                    sync_enable_states(hwnd, h, state);
                }
            });
        }
    });
}

fn handle_copy_command(hwnd: HWND) {
    let cmd = APP_STATE.with(|st| {
        st.try_borrow()
            .ok()
            .and_then(|g| g.as_ref().map(AppState::current_preview_command))
            .unwrap_or_default()
    });
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
    let (use_queue, config_snapshot) = APP_STATE.with(|st| {
        if let Ok(sg) = st.try_borrow()
            && let Some(state) = sg.as_ref()
        {
            return (state.has_enabled_queue_jobs(), Some(state.config.clone()));
        }
        (false, None)
    });
    let Some(cfg) = config_snapshot else {
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
    let logs_dir = APP_STATE.with(|st| {
        st.try_borrow()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.paths.logs_dir.clone()))
    });
    let Some(dir) = logs_dir else {
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
    let info = APP_STATE.with(|st| {
        st.try_borrow().ok().and_then(|g| {
            g.as_ref().map(|s| {
                (
                    s.paths.config_path.to_string_lossy().to_string(),
                    s.paths.logs_dir.to_string_lossy().to_string(),
                    s.is_elevated,
                )
            })
        })
    });
    if let Some((cfg, logs, elev)) = info {
        show_about_dialog(hwnd, &cfg, &logs, elev);
    }
}

fn handle_restart_as_admin(hwnd: HWND) {
    let snapshot = APP_STATE.with(|st| {
        st.try_borrow().ok().and_then(|g| {
            g.as_ref().map(|s| {
                (
                    s.is_busy(),
                    s.is_elevated,
                    s.config.clone(),
                    s.paths.config_path.clone(),
                )
            })
        })
    });
    let Some((busy, elevated, cfg, cfg_path)) = snapshot else {
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

fn handle_start_execution(hwnd: HWND, is_queue: bool, is_dry_run: bool) {
    let prep = APP_STATE.with(|st| {
        st.try_borrow()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.prepare_plan(is_queue, is_dry_run)))
    });
    let Some((plan, validation)) = prep else {
        return;
    };

    if !validation.is_ok() {
        let (focus_field, first_msg) = &validation.errors[0];
        let all_msgs: Vec<String> = validation.errors.iter().map(|(_, m)| m.clone()).collect();
        show_error_box(hwnd, "実行前安全チェックエラー", &all_msgs.join("\n\n"));
        let field = *focus_field;
        let _ = first_msg;
        UI_HANDLES.with(|uh| {
            if let Ok(hg) = uh.try_borrow()
                && let Some(h) = hg.as_ref()
            {
                focus_validation_field(h, field);
            }
        });
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

    let mut init_lines = Vec::new();
    APP_STATE.with(|st| {
        if let Ok(mut sg) = st.try_borrow_mut()
            && let Some(state) = sg.as_mut()
        {
            init_lines = state.begin_execution(plan, hwnd, WM_APP_RUNNER_EVENT);
        }
    });

    UI_HANDLES.with(|uh| {
        if let Ok(hg) = uh.try_borrow()
            && let Some(h) = hg.as_ref()
        {
            set_window_text(h.edit_log, "");
            append_log_lines(h.edit_log, &init_lines);
            switch_to_tab(h, 2);
            APP_STATE.with(|st| {
                if let Ok(mut sg) = st.try_borrow_mut()
                    && let Some(state) = sg.as_mut()
                {
                    state.is_syncing_ui = true;
                    sync_queue_listview(h, state, None);
                    state.is_syncing_ui = false;
                    set_window_text(h.lbl_status, &state.formatted_running_status());
                    sync_enable_states(hwnd, h, state);
                }
            });
        }
    });

    unsafe {
        SetTimer(hwnd, TIMER_ID_STATUS, 500, 0);
    }
}
