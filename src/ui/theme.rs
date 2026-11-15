use std::mem::size_of;

use crate::infra::win32_ffi::*;

/// DPIに応じた通常UIフォントと等幅フォントを保持・管理する。
pub struct ThemeFonts {
    pub ui_font: FontGuard,
    pub mono_font: FontGuard,
    pub bold_font: FontGuard,
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
        if ui_lf.lfHeight == 0 {
            ui_lf.lfHeight = -scale_dpi(12, dpi);
        }
        let ui_hfont = unsafe { CreateFontIndirectW(&raw const ui_lf) };

        let mut bold_lf = ui_lf;
        bold_lf.lfWeight = FW_BOLD;
        let bold_hfont = unsafe { CreateFontIndirectW(&raw const bold_lf) };

        let mono_face = to_wide("Consolas");
        let mono_height = -scale_dpi(12, dpi);
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

        Self {
            ui_font: FontGuard(ui_hfont),
            mono_font: FontGuard(mono_hfont),
            bold_font: FontGuard(bold_hfont),
        }
    }
}
