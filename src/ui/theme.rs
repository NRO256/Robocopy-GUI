use std::mem::size_of;

use crate::infra::win32_ffi::*;

/// タブ内部（SysTabControl32 ペイン）用の純白背景色 (RGB: #FFFFFF)。
pub const COLOR_TAB_BG: COLORREF = 0x00FF_FFFF;
/// コマンドプレビュー・実行ログ欄用の淡色スレート背景色 (RGB: #F8FAFC -> BGR: 0x00FC_FAF8)。
pub const COLOR_CODE_BG: COLORREF = 0x00FC_FAF8;
/// 警告・エラー系テキスト色 (RGB: #B91C1C)。
pub const COLOR_TEXT_DANGER: COLORREF = 0x001C_1CB9;
/// 通常情報・モード解説系テキスト色 (RGB: #0F3A78)。
pub const COLOR_TEXT_INFO: COLORREF = 0x0078_3A0F;
/// 正常・確認OK系テキスト色 (RGB: #156B3D)。
pub const COLOR_TEXT_OK: COLORREF = 0x003D_6B15;

/// DPIに応じた通常UIフォント・等幅フォントおよび共通背景ブラシを保持・管理する。
pub struct ThemeFonts {
    pub ui_font: FontGuard,
    pub mono_font: FontGuard,
    pub bold_font: FontGuard,
    pub tab_bg_brush: BrushGuard,
    pub code_bg_brush: BrushGuard,
}

impl ThemeFonts {
    pub fn for_dpi(dpi: u32) -> Self {
        let dpi = dpi.max(96);
        let mut ncm = NONCLIENTMETRICSW::default();
        let ok = unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS,
                size_of::<NONCLIENTMETRICSW>() as UINT,
                (&raw mut ncm).cast(),
                0,
                dpi,
            )
        };
        if ok == 0 {
            unsafe {
                SystemParametersInfoW(
                    SPI_GETNONCLIENTMETRICS,
                    size_of::<NONCLIENTMETRICSW>() as UINT,
                    (&raw mut ncm).cast(),
                    0,
                );
            }
        }

        let mut ui_lf = ncm.lfMessageFont;
        let target_ui_h = -scale_dpi(13, dpi);
        if ui_lf.lfHeight == 0 || ui_lf.lfHeight.abs() < target_ui_h.abs() {
            ui_lf.lfHeight = target_ui_h;
        }
        let ui_hfont = unsafe { CreateFontIndirectW(&raw const ui_lf) };

        let mut bold_lf = ui_lf;
        bold_lf.lfWeight = FW_BOLD;
        let bold_hfont = unsafe { CreateFontIndirectW(&raw const bold_lf) };

        let mono_face = to_wide("BIZ UDGothic");
        let mono_height = -scale_dpi(13, dpi);
        let mono_hfont = unsafe {
            CreateFontW(
                mono_height,
                0,
                0,
                0,
                FW_NORMAL,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                FIXED_PITCH | FF_MODERN,
                mono_face.as_ptr(),
            )
        };

        let tab_bg_brush = unsafe { CreateSolidBrush(COLOR_TAB_BG) };
        let code_bg_brush = unsafe { CreateSolidBrush(COLOR_CODE_BG) };

        Self {
            ui_font: FontGuard(ui_hfont),
            mono_font: FontGuard(mono_hfont),
            bold_font: FontGuard(bold_hfont),
            tab_bg_brush: BrushGuard(tab_bg_brush),
            code_bg_brush: BrushGuard(code_bg_brush),
        }
    }
}
