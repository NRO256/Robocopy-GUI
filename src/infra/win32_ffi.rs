#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type BYTE = u8;
pub type WORD = u16;
pub type DWORD = u32;
pub type UINT = u32;
pub type INT = i32;
pub type LONG = i32;
pub type HRESULT = i32;
pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type ATOM = u16;
pub type HWND = isize;
pub type HMENU = isize;
pub type HINSTANCE = isize;
pub type HICON = isize;
pub type HCURSOR = isize;
pub type HBRUSH = isize;
pub type HFONT = isize;
pub type HDC = isize;
pub type HANDLE = isize;
pub type HGLOBAL = isize;
pub type HDROP = isize;
pub type COLORREF = u32;

pub const FALSE: BOOL = 0;
pub const TRUE: BOOL = 1;
pub const INVALID_HANDLE_VALUE: HANDLE = -1;
pub const ERROR_ALREADY_EXISTS: DWORD = 183;
pub const WAIT_OBJECT_0: DWORD = 0;

pub const CP_ACP: UINT = 0;

// Window / Class styles
pub const CS_HREDRAW: UINT = 0x0002;
pub const CS_VREDRAW: UINT = 0x0001;
pub const WS_OVERLAPPED: DWORD = 0x0000_0000;
pub const WS_POPUP: DWORD = 0x8000_0000;
pub const WS_CHILD: DWORD = 0x4000_0000;
pub const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
pub const WS_MAXIMIZEBOX: DWORD = 0x0001_0000;
pub const WS_CAPTION: DWORD = 0x00C0_0000;
pub const WS_SYSMENU: DWORD = 0x0008_0000;
pub const WS_THICKFRAME: DWORD = 0x0004_0000;
pub const WS_OVERLAPPEDWINDOW: DWORD =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
pub const WS_VISIBLE: DWORD = 0x1000_0000;
pub const WS_CLIPCHILDREN: DWORD = 0x0200_0000;
pub const WS_CLIPSIBLINGS: DWORD = 0x0400_0000;
pub const WS_TABSTOP: DWORD = 0x0001_0000;
pub const WS_GROUP: DWORD = 0x0002_0000;
pub const WS_VSCROLL: DWORD = 0x0020_0000;
pub const WS_HSCROLL: DWORD = 0x0010_0000;
pub const WS_BORDER: DWORD = 0x0080_0000;
pub const WS_EX_CLIENTEDGE: DWORD = 0x0000_0200;
pub const WS_EX_CONTROLPARENT: DWORD = 0x0001_0000;
pub const WS_EX_DLGMODALFRAME: DWORD = 0x0000_0001;
pub const CW_USEDEFAULT: INT = -0x8000_0000_i32;

// Button / Edit / Static / Combo / Tab / ListView styles
pub const BS_PUSHBUTTON: DWORD = 0x0000_0000;
pub const BS_DEFPUSHBUTTON: DWORD = 0x0000_0001;
pub const BS_AUTOCHECKBOX: DWORD = 0x0000_0003;
pub const BS_AUTORADIOBUTTON: DWORD = 0x0000_0009;
pub const BS_GROUPBOX: DWORD = 0x0000_0007;
pub const BM_GETCHECK: UINT = 0x00F0;
pub const BM_SETCHECK: UINT = 0x00F1;
pub const BST_UNCHECKED: WPARAM = 0;
pub const BST_CHECKED: WPARAM = 1;
pub const BN_CLICKED: WORD = 0;

pub const SS_LEFT: DWORD = 0x0000_0000;
pub const SS_NOPREFIX: DWORD = 0x0000_0080;

pub const ES_MULTILINE: DWORD = 0x0004;
pub const ES_AUTOVSCROLL: DWORD = 0x0040;
pub const ES_AUTOHSCROLL: DWORD = 0x0080;
pub const ES_READONLY: DWORD = 0x0800;
pub const ES_NUMBER: DWORD = 0x2000;
pub const EN_CHANGE: WORD = 0x0300;
pub const EM_GETSEL: UINT = 0x00B0;
pub const EM_SETSEL: UINT = 0x00B1;
pub const EM_REPLACESEL: UINT = 0x00C2;
pub const EM_GETLINECOUNT: UINT = 0x00BA;
pub const EM_LINEINDEX: UINT = 0x00BB;
pub const EM_SETLIMITTEXT: UINT = 0x00C5;
pub const EM_SCROLLCARET: UINT = 0x00B7;
pub const EM_SETCUEBANNER: UINT = 0x1501;

pub const CBS_DROPDOWNLIST: DWORD = 0x0003;
pub const CBN_SELCHANGE: WORD = 1;
pub const CB_ADDSTRING: UINT = 0x0143;
pub const CB_RESETCONTENT: UINT = 0x014B;
pub const CB_GETCURSEL: UINT = 0x0147;
pub const CB_SETCURSEL: UINT = 0x014E;

// Tab control
pub const TCM_INSERTITEMW: UINT = 0x133E;
pub const TCM_GETCURSEL: UINT = 0x130B;
pub const TCM_SETCURSEL: UINT = 0x130C;
pub const TCM_ADJUSTRECT: UINT = 0x1328;
pub const TCIF_TEXT: UINT = 0x0001;
pub const TCN_SELCHANGE: u32 = (-551_i32) as u32;

// ListView
pub const LVS_REPORT: DWORD = 0x0001;
pub const LVS_SINGLESEL: DWORD = 0x0004;
pub const LVS_SHOWSELALWAYS: DWORD = 0x0008;
pub const LVS_EX_GRIDLINES: DWORD = 0x0000_0001;
pub const LVS_EX_CHECKBOXES: DWORD = 0x0000_0004;
pub const LVS_EX_FULLROWSELECT: DWORD = 0x0000_0020;
pub const LVS_EX_DOUBLEBUFFER: DWORD = 0x0001_0000;
pub const LVM_FIRST: UINT = 0x1000;
pub const LVM_DELETEALLITEMS: UINT = LVM_FIRST + 9;
pub const LVM_GETNEXTITEM: UINT = LVM_FIRST + 12;
pub const LVM_SETCOLUMNWIDTH: UINT = LVM_FIRST + 30;
pub const LVM_SETITEMSTATE: UINT = LVM_FIRST + 43;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: UINT = LVM_FIRST + 54;
pub const LVM_INSERTITEMW: UINT = LVM_FIRST + 77;
pub const LVM_SETITEMTEXTW: UINT = LVM_FIRST + 116;
pub const LVM_INSERTCOLUMNW: UINT = LVM_FIRST + 97;
pub const LVM_GETITEMRECT: UINT = LVM_FIRST + 14;
pub const LVM_HITTEST: UINT = LVM_FIRST + 18;
pub const LVM_ENSUREVISIBLE: UINT = LVM_FIRST + 19;
pub const LVIR_BOUNDS: INT = 0;
pub const LVNI_SELECTED: LPARAM = 0x0002;
pub const LVIF_TEXT: UINT = 0x0001;
pub const LVIF_STATE: UINT = 0x0008;
pub const LVCF_FMT: UINT = 0x0001;
pub const LVCF_WIDTH: UINT = 0x0002;
pub const LVCF_TEXT: UINT = 0x0004;
pub const LVCFMT_LEFT: INT = 0x0000;
pub const LVIS_SELECTED: UINT = 0x0002;
pub const LVIS_FOCUSED: UINT = 0x0001;
pub const LVIS_STATEIMAGEMASK: UINT = 0xF000;
pub const NM_DBLCLK: u32 = (-3_i32) as u32;
pub const LVN_ITEMCHANGED: u32 = (-101_i32) as u32;
pub const LVN_KEYDOWN: u32 = (-155_i32) as u32;

// Messages
pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_SETFONT: UINT = 0x0030;
pub const WM_SETICON: UINT = 0x0080;
pub const WM_GETMINMAXINFO: UINT = 0x0024;
pub const WM_NOTIFY: UINT = 0x004E;
pub const WM_CONTEXTMENU: UINT = 0x007B;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_CTLCOLOREDIT: UINT = 0x0133;
pub const WM_CTLCOLORBTN: UINT = 0x0135;
pub const WM_CTLCOLORSTATIC: UINT = 0x0138;
pub const WM_DROPFILES: UINT = 0x0233;
pub const WM_DPICHANGED: UINT = 0x02E0;
pub const WM_APP: UINT = 0x8000;

pub const ICON_SMALL: WPARAM = 0;
pub const ICON_BIG: WPARAM = 1;

pub const VK_TAB: WPARAM = 0x09;
pub const VK_SHIFT: INT = 0x10;
pub const VK_CONTROL: INT = 0x11;
pub const VK_MENU: INT = 0x12;
pub const VK_UP: WORD = 0x26;
pub const VK_DOWN: WORD = 0x28;
pub const VK_DELETE: WORD = 0x2E;

// ShowWindow / SetWindowPos / Colors / Icons
pub const SW_HIDE: INT = 0;
pub const SW_SHOW: INT = 5;
pub const SW_RESTORE: INT = 9;
pub const HWND_BOTTOM: HWND = 1;
pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOZORDER: UINT = 0x0004;
pub const SWP_NOACTIVATE: UINT = 0x0010;
pub const GWLP_USERDATA: INT = -21;
pub const COLOR_WINDOW: INT = 5;
pub const COLOR_BTNFACE: INT = 15;
pub const COLOR_WINDOWTEXT: INT = 8;
pub const TRANSPARENT: INT = 1;
pub const OPAQUE: INT = 2;
pub const IDI_APPLICATION: usize = 32512;
pub const IDC_ARROW: usize = 32512;
pub const SPI_GETNONCLIENTMETRICS: UINT = 0x0029;
pub const LF_FACESIZE: usize = 32;
pub const FW_NORMAL: INT = 400;
pub const FW_BOLD: INT = 700;
pub const DEFAULT_CHARSET: DWORD = 1;
pub const OUT_DEFAULT_PRECIS: DWORD = 0;
pub const CLIP_DEFAULT_PRECIS: DWORD = 0;
pub const CLEARTYPE_QUALITY: DWORD = 5;
pub const FIXED_PITCH: DWORD = 1;
pub const FF_MODERN: DWORD = 3 << 4;

// Menu
pub const MF_STRING: UINT = 0x0000;
pub const MF_SEPARATOR: UINT = 0x0800;
pub const MF_POPUP: UINT = 0x0010;
pub const MF_ENABLED: UINT = 0x0000;
pub const MF_GRAYED: UINT = 0x0001;
pub const MF_CHECKED: UINT = 0x0008;
pub const MF_UNCHECKED: UINT = 0x0000;
pub const MF_BYCOMMAND: UINT = 0x0000;
pub const MF_BYPOSITION: UINT = 0x0400;
pub const TPM_LEFTALIGN: UINT = 0x0000;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;
pub const TPM_RETURNCMD: UINT = 0x0100;

// MessageBox / TaskDialog
pub const MB_OK: UINT = 0x0000_0000;
pub const MB_YESNO: UINT = 0x0000_0004;
pub const MB_ICONINFORMATION: UINT = 0x0000_0040;
pub const MB_ICONEXCLAMATION: UINT = 0x0000_0030;
pub const MB_ICONERROR: UINT = 0x0000_0010;
pub const MB_DEFBUTTON2: UINT = 0x0000_0100;
pub const IDOK: INT = 1;
pub const IDCANCEL: INT = 2;
pub const IDYES: INT = 6;
pub const IDNO: INT = 7;

pub const TDF_ALLOW_DIALOG_CANCELLATION: INT = 0x0008;
pub const TDF_EXPAND_FOOTER_AREA: INT = 0x0040;
pub const TDF_SHOW_PROGRESS_BAR: INT = 0x0200;
pub const TDF_CALLBACK_TIMER: INT = 0x0800;
pub const TDF_POSITION_RELATIVE_TO_WINDOW: INT = 0x1000;
pub const TDCBF_OK_BUTTON: INT = 0x0001;
pub const TDCBF_CANCEL_BUTTON: INT = 0x0008;
const fn make_int_resource_w(id: i16) -> *const u16 {
    (id as u16) as usize as *const u16
}

pub const TD_WARNING_ICON: *const u16 = make_int_resource_w(-1);
pub const TD_ERROR_ICON: *const u16 = make_int_resource_w(-2);
pub const TD_INFORMATION_ICON: *const u16 = make_int_resource_w(-3);
#[cfg(test)]
pub const TD_SHIELD_ICON: *const u16 = make_int_resource_w(-4);
pub const TDN_CREATED: UINT = 0;
pub const TDN_BUTTON_CLICKED: UINT = 2;
pub const TDN_TIMER: UINT = 4;
pub const TDM_CLICK_BUTTON: UINT = 0x0400 + 102;
pub const TDM_SET_PROGRESS_BAR_RANGE: UINT = 0x0400 + 105;
pub const TDM_SET_PROGRESS_BAR_POS: UINT = 0x0400 + 106;
pub const TDM_SET_ELEMENT_TEXT: UINT = 0x0400 + 108;
pub const TDE_CONTENT: WPARAM = 0;
pub const TDE_FOOTER: WPARAM = 2;

// Common controls / DPI / Clipboard / Execution state / Progress / Tooltips / DWM
pub const ICC_STANDARD_CLASSES: DWORD = 0x0000_4000;
pub const ICC_LISTVIEW_CLASSES: DWORD = 0x0000_0001;
pub const ICC_TAB_CLASSES: DWORD = 0x0000_0008;
pub const ICC_PROGRESS_CLASS: DWORD = 0x0000_0020;
pub const PBS_SMOOTH: DWORD = 0x01;
pub const PBM_SETPOS: UINT = 0x0400 + 2;
pub const PBM_SETRANGE32: UINT = 0x0400 + 6;
pub const PBM_SETSTATE: UINT = 0x0400 + 16;
pub const PBST_NORMAL: WPARAM = 0x0001;
pub const PBST_ERROR: WPARAM = 0x0002;
pub const PBST_PAUSED: WPARAM = 0x0003;
pub const TTS_ALWAYSTIP: DWORD = 0x01;
pub const TTS_NOPREFIX: DWORD = 0x02;
pub const TTF_IDISHWND: UINT = 0x0001;
pub const TTF_SUBCLASS: UINT = 0x0010;
pub const TTM_SETMAXTIPWIDTH: UINT = 0x0400 + 24;
pub const TTM_ADDTOOLW: UINT = 0x0400 + 50;
pub const DWMWA_WINDOW_CORNER_PREFERENCE: DWORD = 33;
pub const DWMWCP_ROUND: DWORD = 2;
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
pub const CF_UNICODETEXT: UINT = 13;
pub const GMEM_MOVEABLE: UINT = 0x0002;
pub const ES_SYSTEM_REQUIRED: DWORD = 0x0000_0001;
pub const ES_CONTINUOUS: DWORD = 0x8000_0000;

// File / Process / JobObject / Token / Shell / Dialogs
pub const FILE_READ_ATTRIBUTES: DWORD = 0x0080;
pub const FILE_SHARE_READ: DWORD = 0x0000_0001;
pub const FILE_SHARE_WRITE: DWORD = 0x0000_0002;
pub const FILE_SHARE_DELETE: DWORD = 0x0000_0004;
pub const OPEN_EXISTING: DWORD = 3;
pub const FILE_FLAG_BACKUP_SEMANTICS: DWORD = 0x0200_0000;
pub const FILE_ATTRIBUTE_REPARSE_POINT: DWORD = 0x0000_0400;
pub const INVALID_FILE_ATTRIBUTES: DWORD = 0xFFFF_FFFF;
pub const MOVEFILE_REPLACE_EXISTING: DWORD = 0x0000_0001;
pub const MOVEFILE_WRITE_THROUGH: DWORD = 0x0000_0008;

pub const CREATE_SUSPENDED: DWORD = 0x0000_0004;
pub const CREATE_NO_WINDOW: DWORD = 0x0800_0000;
pub const STARTF_USESTDHANDLES: DWORD = 0x0000_0100;
pub const HANDLE_FLAG_INHERIT: DWORD = 0x0000_0001;
pub const JobObjectExtendedLimitInformation: INT = 9;
pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x0000_2000;
pub const TOKEN_QUERY: DWORD = 0x0008;
pub const TokenElevation: INT = 20;
pub const CSTR_EQUAL: INT = 2;

// OpenFileName / BrowseForFolder
pub const OFN_OVERWRITEPROMPT: DWORD = 0x0000_0002;
pub const OFN_PATHMUSTEXIST: DWORD = 0x0000_0800;
pub const OFN_NOCHANGEDIR: DWORD = 0x0000_0008;
pub const BIF_RETURNONLYFSDIRS: UINT = 0x0000_0001;
pub const BIF_NEWDIALOGSTYLE: UINT = 0x0000_0040;
pub const BIF_EDITBOX: UINT = 0x0000_0010;
pub const BFFM_INITIALIZED: UINT = 1;
pub const BFFM_SETSELECTIONW: UINT = 0x0400 + 103;
pub const COINIT_APARTMENTTHREADED: DWORD = 0x2;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

#[repr(C)]
#[derive(Default)]
pub struct LVHITTESTINFO {
    pub pt: POINT,
    pub flags: UINT,
    pub iItem: INT,
    pub iSubItem: INT,
    pub iGroup: INT,
}

#[repr(C)]
#[derive(Default)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

#[repr(C)]
#[derive(Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

pub type WNDPROC = Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT>;
pub type PFTASKDIALOGCALLBACK =
    Option<unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM, isize) -> HRESULT>;

#[repr(C)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: WNDPROC,
    pub cbClsExtra: INT,
    pub cbWndExtra: INT,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: HICON,
}

impl Default for WNDCLASSEXW {
    fn default() -> Self {
        Self {
            cbSize: std::mem::size_of::<Self>() as UINT,
            style: 0,
            lpfnWndProc: None,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: 0,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: std::ptr::null(),
            hIconSm: 0,
        }
    }
}

#[repr(C)]
#[derive(Default)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: DWORD,
    pub dwICC: DWORD,
}

#[repr(C)]
pub struct LOGFONTW {
    pub lfHeight: LONG,
    pub lfWidth: LONG,
    pub lfEscapement: LONG,
    pub lfOrientation: LONG,
    pub lfWeight: LONG,
    pub lfItalic: BYTE,
    pub lfUnderline: BYTE,
    pub lfStrikeOut: BYTE,
    pub lfCharSet: BYTE,
    pub lfOutPrecision: BYTE,
    pub lfClipPrecision: BYTE,
    pub lfQuality: BYTE,
    pub lfPitchAndFamily: BYTE,
    pub lfFaceName: [u16; LF_FACESIZE],
}

impl Default for LOGFONTW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct NONCLIENTMETRICSW {
    pub cbSize: UINT,
    pub iBorderWidth: INT,
    pub iScrollWidth: INT,
    pub iScrollHeight: INT,
    pub iCaptionWidth: INT,
    pub iCaptionHeight: INT,
    pub lfCaptionFont: LOGFONTW,
    pub iSmCaptionWidth: INT,
    pub iSmCaptionHeight: INT,
    pub lfSmCaptionFont: LOGFONTW,
    pub iMenuWidth: INT,
    pub iMenuHeight: INT,
    pub lfMenuFont: LOGFONTW,
    pub lfStatusFont: LOGFONTW,
    pub lfMessageFont: LOGFONTW,
    pub iPaddedBorderWidth: INT,
}

impl Default for NONCLIENTMETRICSW {
    fn default() -> Self {
        let mut val: Self = unsafe { std::mem::zeroed() };
        val.cbSize = std::mem::size_of::<Self>() as UINT;
        val
    }
}

#[repr(C)]
#[derive(Default)]
pub struct NMHDR {
    pub hwndFrom: HWND,
    pub idFrom: usize,
    pub code: UINT,
}

#[repr(C)]
#[derive(Default)]
pub struct NMLISTVIEW {
    pub hdr: NMHDR,
    pub iItem: INT,
    pub iSubItem: INT,
    pub uNewState: UINT,
    pub uOldState: UINT,
    pub uChanged: UINT,
    pub ptAction: POINT,
    pub lParam: LPARAM,
}

#[repr(C, packed(1))]
#[derive(Default)]
pub struct NMLVKEYDOWN {
    pub hdr: NMHDR,
    pub wVKey: WORD,
    pub flags: UINT,
}

#[repr(C)]
pub struct TTTOOLINFOW {
    pub cbSize: UINT,
    pub uFlags: UINT,
    pub hwnd: HWND,
    pub uId: usize,
    pub rect: RECT,
    pub hinst: HINSTANCE,
    pub lpszText: *mut u16,
    pub lParam: LPARAM,
}

impl Default for TTTOOLINFOW {
    fn default() -> Self {
        let mut ti: Self = unsafe { std::mem::zeroed() };
        ti.cbSize = std::mem::size_of::<Self>() as UINT;
        ti
    }
}

#[repr(C)]
pub struct TCITEMW {
    pub mask: UINT,
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub pszText: *mut u16,
    pub cchTextMax: INT,
    pub iImage: INT,
    pub lParam: LPARAM,
}

impl Default for TCITEMW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct LVCOLUMNW {
    pub mask: UINT,
    pub fmt: INT,
    pub cx: INT,
    pub pszText: *mut u16,
    pub cchTextMax: INT,
    pub iSubItem: INT,
    pub iImage: INT,
    pub iOrder: INT,
    pub cxMin: INT,
    pub cxDefault: INT,
    pub cxIdeal: INT,
}

impl Default for LVCOLUMNW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct LVITEMW {
    pub mask: UINT,
    pub iItem: INT,
    pub iSubItem: INT,
    pub state: UINT,
    pub stateMask: UINT,
    pub pszText: *mut u16,
    pub cchTextMax: INT,
    pub iImage: INT,
    pub lParam: LPARAM,
    pub iIndent: INT,
    pub iGroupId: INT,
    pub cColumns: UINT,
    pub puColumns: *mut UINT,
    pub piColFmt: *mut INT,
    pub iGroup: INT,
}

impl Default for LVITEMW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C, packed(1))]
pub struct TASKDIALOG_BUTTON {
    pub nButtonID: INT,
    pub pszButtonText: *const u16,
}

#[repr(C, packed(1))]
pub struct TASKDIALOGCONFIG {
    pub cbSize: UINT,
    pub hwndParent: HWND,
    pub hInstance: HINSTANCE,
    pub dwFlags: INT,
    pub dwCommonButtons: INT,
    pub pszWindowTitle: *const u16,
    pub hMainIcon: *const u16,
    pub pszMainInstruction: *const u16,
    pub pszContent: *const u16,
    pub cButtons: UINT,
    pub pButtons: *const TASKDIALOG_BUTTON,
    pub nDefaultButton: INT,
    pub cRadioButtons: UINT,
    pub pRadioButtons: *const TASKDIALOG_BUTTON,
    pub nDefaultRadioButton: INT,
    pub pszVerificationText: *const u16,
    pub pszExpandedInformation: *const u16,
    pub pszExpandedControlText: *const u16,
    pub pszCollapsedControlText: *const u16,
    pub hFooterIcon: *const u16,
    pub pszFooter: *const u16,
    pub pfCallback: PFTASKDIALOGCALLBACK,
    pub lpCallbackData: isize,
    pub cxWidth: UINT,
}

impl Default for TASKDIALOGCONFIG {
    fn default() -> Self {
        let mut cfg: Self = unsafe { std::mem::zeroed() };
        cfg.cbSize = std::mem::size_of::<Self>() as UINT;
        cfg
    }
}

#[repr(C)]
pub struct OPENFILENAMEW {
    pub lStructSize: DWORD,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: *const u16,
    pub lpstrCustomFilter: *mut u16,
    pub nMaxCustFilter: DWORD,
    pub nFilterIndex: DWORD,
    pub lpstrFile: *mut u16,
    pub nMaxFile: DWORD,
    pub lpstrFileTitle: *mut u16,
    pub nMaxFileTitle: DWORD,
    pub lpstrInitialDir: *const u16,
    pub lpstrTitle: *const u16,
    pub Flags: DWORD,
    pub nFileOffset: WORD,
    pub nFileExtension: WORD,
    pub lpstrDefExt: *const u16,
    pub lCustData: LPARAM,
    pub lpfnHook: usize,
    pub lpTemplateName: *const u16,
    pub pvReserved: *mut c_void,
    pub dwReserved: DWORD,
    pub FlagsEx: DWORD,
}

impl Default for OPENFILENAMEW {
    fn default() -> Self {
        let mut ofn: Self = unsafe { std::mem::zeroed() };
        ofn.lStructSize = std::mem::size_of::<Self>() as DWORD;
        ofn
    }
}

pub type BFFCALLBACK = Option<unsafe extern "system" fn(HWND, UINT, LPARAM, LPARAM) -> INT>;

#[repr(C)]
pub struct BROWSEINFOW {
    pub hwndOwner: HWND,
    pub pidlRoot: *const c_void,
    pub pszDisplayName: *mut u16,
    pub lpszTitle: *const u16,
    pub ulFlags: UINT,
    pub lpfn: BFFCALLBACK,
    pub lParam: LPARAM,
    pub iImage: INT,
}

impl Default for BROWSEINFOW {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: DWORD,
    pub lpSecurityDescriptor: *mut c_void,
    pub bInheritHandle: BOOL,
}

#[repr(C)]
pub struct STARTUPINFOW {
    pub cb: DWORD,
    pub lpReserved: *mut u16,
    pub lpDesktop: *mut u16,
    pub lpTitle: *mut u16,
    pub dwX: DWORD,
    pub dwY: DWORD,
    pub dwXSize: DWORD,
    pub dwYSize: DWORD,
    pub dwXCountChars: DWORD,
    pub dwYCountChars: DWORD,
    pub dwFillAttribute: DWORD,
    pub dwFlags: DWORD,
    pub wShowWindow: WORD,
    pub cbReserved2: WORD,
    pub lpReserved2: *mut BYTE,
    pub hStdInput: HANDLE,
    pub hStdOutput: HANDLE,
    pub hStdError: HANDLE,
}

impl Default for STARTUPINFOW {
    fn default() -> Self {
        let mut si: Self = unsafe { std::mem::zeroed() };
        si.cb = std::mem::size_of::<Self>() as DWORD;
        si
    }
}

#[repr(C)]
#[derive(Default)]
pub struct PROCESS_INFORMATION {
    pub hProcess: HANDLE,
    pub hThread: HANDLE,
    pub dwProcessId: DWORD,
    pub dwThreadId: DWORD,
}

#[repr(C)]
#[derive(Default)]
pub struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    pub PerProcessUserTimeLimit: i64,
    pub PerJobUserTimeLimit: i64,
    pub LimitFlags: DWORD,
    pub MinimumWorkingSetSize: usize,
    pub MaximumWorkingSetSize: usize,
    pub ActiveProcessLimit: DWORD,
    pub Affinity: usize,
    pub PriorityClass: DWORD,
    pub SchedulingClass: DWORD,
}

#[repr(C)]
#[derive(Default)]
pub struct IO_COUNTERS {
    pub ReadOperationCount: u64,
    pub WriteOperationCount: u64,
    pub OtherOperationCount: u64,
    pub ReadTransferCount: u64,
    pub WriteTransferCount: u64,
    pub OtherTransferCount: u64,
}

#[repr(C)]
#[derive(Default)]
pub struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    pub BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    pub IoInfo: IO_COUNTERS,
    pub ProcessMemoryLimit: usize,
    pub JobMemoryLimit: usize,
    pub PeakProcessMemoryUsed: usize,
    pub PeakJobMemoryUsed: usize,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct SYSTEMTIME {
    pub wYear: WORD,
    pub wMonth: WORD,
    pub wDayOfWeek: WORD,
    pub wDay: WORD,
    pub wHour: WORD,
    pub wMinute: WORD,
    pub wSecond: WORD,
    pub wMilliseconds: WORD,
}

#[repr(C)]
#[derive(Default)]
pub struct TOKEN_ELEVATION {
    pub TokenIsElevated: DWORD,
}

#[link(name = "user32")]
unsafe extern "system" {
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: DWORD,
        x: INT,
        y: INT,
        nWidth: INT,
        nHeight: INT,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: INT) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn IsIconic(hWnd: HWND) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> HWND;
    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;
    pub fn IsDialogMessageW(hDlg: HWND, lpMsg: *const MSG) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: INT);
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> BOOL;
    pub fn GetWindowTextLengthW(hWnd: HWND) -> INT;
    pub fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: INT) -> INT;
    pub fn EnableWindow(hWnd: HWND, bEnable: BOOL) -> BOOL;
    pub fn IsWindowEnabled(hWnd: HWND) -> BOOL;
    pub fn SetFocus(hWnd: HWND) -> HWND;
    pub fn GetFocus() -> HWND;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: INT) -> isize;
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: INT, dwNewLong: isize) -> isize;
    pub fn MoveWindow(
        hWnd: HWND,
        X: INT,
        Y: INT,
        nWidth: INT,
        nHeight: INT,
        bRepaint: BOOL,
    ) -> BOOL;
    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: INT,
        Y: INT,
        cx: INT,
        cy: INT,
        uFlags: UINT,
    ) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn SetProcessDpiAwarenessContext(value: isize) -> BOOL;
    pub fn GetDpiForWindow(hwnd: HWND) -> UINT;
    pub fn GetDpiForSystem() -> UINT;
    pub fn SystemParametersInfoW(
        uiAction: UINT,
        uiParam: UINT,
        pvParam: *mut c_void,
        fWinIni: UINT,
    ) -> BOOL;
    pub fn SystemParametersInfoForDpi(
        uiAction: UINT,
        uiParam: UINT,
        pvParam: *mut c_void,
        fWinIni: UINT,
        dpi: UINT,
    ) -> BOOL;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: *const u16) -> HICON;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    pub fn GetSysColorBrush(nIndex: INT) -> HBRUSH;
    pub fn GetSysColor(nIndex: INT) -> DWORD;
    pub fn MessageBoxW(hWnd: HWND, lpText: *const u16, lpCaption: *const u16, uType: UINT) -> INT;
    pub fn MessageBeep(uType: UINT) -> BOOL;
    pub fn SetTimer(hWnd: HWND, nIDEvent: usize, uElapse: UINT, lpTimerFunc: usize) -> usize;
    pub fn KillTimer(hWnd: HWND, uIDEvent: usize) -> BOOL;
    pub fn GetKeyState(nVirtKey: INT) -> i16;
    pub fn CreateMenu() -> HMENU;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn AppendMenuW(
        hMenu: HMENU,
        uFlags: UINT,
        uIDNewItem: usize,
        lpNewItem: *const u16,
    ) -> BOOL;
    pub fn SetMenu(hWnd: HWND, hMenu: HMENU) -> BOOL;
    pub fn GetMenuItemCount(hMenu: HMENU) -> INT;
    pub fn DeleteMenu(hMenu: HMENU, uPosition: UINT, uFlags: UINT) -> BOOL;
    pub fn EnableMenuItem(hMenu: HMENU, uIDEnableItem: UINT, uEnable: UINT) -> BOOL;
    pub fn CheckMenuItem(hMenu: HMENU, uIDCheckItem: UINT, uCheck: UINT) -> DWORD;
    pub fn DrawMenuBar(hWnd: HWND) -> BOOL;
    pub fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: INT,
        y: INT,
        nReserved: INT,
        hWnd: HWND,
        prcRect: *const RECT,
    ) -> BOOL;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
    pub fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
    pub fn EmptyClipboard() -> BOOL;
    pub fn SetClipboardData(uFormat: UINT, hMem: HANDLE) -> HANDLE;
    pub fn CloseClipboard() -> BOOL;
    pub fn DestroyIcon(hIcon: HICON) -> BOOL;
    pub fn PtInRect(lprc: *const RECT, pt: POINT) -> BOOL;
    pub fn GetClassNameW(hWnd: HWND, lpClassName: *mut u16, nMaxCount: INT) -> INT;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub fn CreateFontIndirectW(lplf: *const LOGFONTW) -> HFONT;
    pub fn CreateFontW(
        cHeight: INT,
        cWidth: INT,
        cEscapement: INT,
        cOrientation: INT,
        cWeight: INT,
        bItalic: DWORD,
        bUnderline: DWORD,
        bStrikeOut: DWORD,
        iCharSet: DWORD,
        iOutPrecision: DWORD,
        iClipPrecision: DWORD,
        iQuality: DWORD,
        iPitchAndFamily: DWORD,
        pszFaceName: *const u16,
    ) -> HFONT;
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn DeleteObject(ho: HFONT) -> BOOL;
    pub fn SetBkMode(hdc: HDC, mode: INT) -> INT;
    pub fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    pub fn InitCommonControlsEx(picce: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn TaskDialogIndirect(
        pTaskConfig: *const TASKDIALOGCONFIG,
        pnButton: *mut INT,
        pnRadioButton: *mut INT,
        pfVerificationFlagChecked: *mut BOOL,
    ) -> HRESULT;
}

#[link(name = "comdlg32")]
unsafe extern "system" {
    pub fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> BOOL;
}

#[link(name = "shell32")]
unsafe extern "system" {
    pub fn ShellExecuteW(
        hwnd: HWND,
        lpOperation: *const u16,
        lpFile: *const u16,
        lpParameters: *const u16,
        lpDirectory: *const u16,
        nShowCmd: INT,
    ) -> HINSTANCE;
    pub fn SHBrowseForFolderW(lpbi: *mut BROWSEINFOW) -> *mut c_void;
    pub fn SHGetPathFromIDListW(pidl: *const c_void, pszPath: *mut u16) -> BOOL;
    pub fn DragAcceptFiles(hWnd: HWND, fAccept: BOOL);
    pub fn DragQueryFileW(hDrop: HDROP, iFile: UINT, lpszFile: *mut u16, cch: UINT) -> UINT;
    pub fn DragQueryPoint(hDrop: HDROP, lppt: *mut POINT) -> BOOL;
    pub fn DragFinish(hDrop: HDROP);
    pub fn ExtractIconExW(
        lpszFile: *const u16,
        nIconIndex: INT,
        phiconLarge: *mut HICON,
        phiconSmall: *mut HICON,
        nIcons: UINT,
    ) -> UINT;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    pub fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: DWORD,
        pvAttribute: *const c_void,
        cbAttribute: DWORD,
    ) -> HRESULT;
}

#[link(name = "uxtheme")]
unsafe extern "system" {
    pub fn SetWindowTheme(
        hwnd: HWND,
        pszSubAppName: *const u16,
        pszSubIdList: *const u16,
    ) -> HRESULT;
}

#[link(name = "ole32")]
unsafe extern "system" {
    pub fn CoTaskMemFree(pv: *mut c_void);
    pub fn CoInitializeEx(pvReserved: *mut c_void, dwCoInit: DWORD) -> HRESULT;
    pub fn CoUninitialize();
}

#[link(name = "advapi32")]
unsafe extern "system" {
    pub fn OpenProcessToken(
        ProcessHandle: HANDLE,
        DesiredAccess: DWORD,
        TokenHandle: *mut HANDLE,
    ) -> BOOL;
    pub fn GetTokenInformation(
        TokenHandle: HANDLE,
        TokenInformationClass: INT,
        TokenInformation: *mut c_void,
        TokenInformationLength: DWORD,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
}

#[link(name = "mpr")]
unsafe extern "system" {
    pub fn WNetGetConnectionW(
        lpLocalName: *const u16,
        lpRemoteName: *mut u16,
        lpnLength: *mut DWORD,
    ) -> DWORD;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
    pub fn GetModuleFileNameW(hModule: HINSTANCE, lpFilename: *mut u16, nSize: DWORD) -> DWORD;
    pub fn GetSystemDirectoryW(lpBuffer: *mut u16, uSize: UINT) -> UINT;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetLastError() -> DWORD;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn CreateMutexW(
        lpMutexAttributes: *mut SECURITY_ATTRIBUTES,
        bInitialOwner: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
    pub fn CreateJobObjectW(
        lpJobAttributes: *mut SECURITY_ATTRIBUTES,
        lpName: *const u16,
    ) -> HANDLE;
    pub fn SetInformationJobObject(
        hJob: HANDLE,
        JobObjectInformationClass: INT,
        lpJobObjectInformation: *const c_void,
        cbJobObjectInformationLength: DWORD,
    ) -> BOOL;
    pub fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;
    pub fn TerminateJobObject(hJob: HANDLE, uExitCode: UINT) -> BOOL;
    pub fn CreateProcessW(
        lpApplicationName: *const u16,
        lpCommandLine: *mut u16,
        lpProcessAttributes: *mut SECURITY_ATTRIBUTES,
        lpThreadAttributes: *mut SECURITY_ATTRIBUTES,
        bInheritHandles: BOOL,
        dwCreationFlags: DWORD,
        lpEnvironment: *mut c_void,
        lpCurrentDirectory: *const u16,
        lpStartupInfo: *const STARTUPINFOW,
        lpProcessInformation: *mut PROCESS_INFORMATION,
    ) -> BOOL;
    pub fn ResumeThread(hThread: HANDLE) -> DWORD;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: UINT) -> BOOL;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn GetExitCodeProcess(hProcess: HANDLE, lpExitCode: *mut DWORD) -> BOOL;
    pub fn CreatePipe(
        hReadPipe: *mut HANDLE,
        hWritePipe: *mut HANDLE,
        lpPipeAttributes: *mut SECURITY_ATTRIBUTES,
        nSize: DWORD,
    ) -> BOOL;
    pub fn SetHandleInformation(hObject: HANDLE, dwMask: DWORD, dwFlags: DWORD) -> BOOL;
    pub fn ReadFile(
        hFile: HANDLE,
        lpBuffer: *mut c_void,
        nNumberOfBytesToRead: DWORD,
        lpNumberOfBytesRead: *mut DWORD,
        lpOverlapped: *mut c_void,
    ) -> BOOL;
    pub fn CreateFileW(
        lpFileName: *const u16,
        dwDesiredAccess: DWORD,
        dwShareMode: DWORD,
        lpSecurityAttributes: *mut SECURITY_ATTRIBUTES,
        dwCreationDisposition: DWORD,
        dwFlagsAndAttributes: DWORD,
        hTemplateFile: HANDLE,
    ) -> HANDLE;
    pub fn GetFinalPathNameByHandleW(
        hFile: HANDLE,
        lpszFilePath: *mut u16,
        cchFilePath: DWORD,
        dwFlags: DWORD,
    ) -> DWORD;
    pub fn GetFileAttributesW(lpFileName: *const u16) -> DWORD;
    pub fn MoveFileExW(
        lpExistingFileName: *const u16,
        lpNewFileName: *const u16,
        dwFlags: DWORD,
    ) -> BOOL;
    pub fn CompareStringOrdinal(
        lpString1: *const u16,
        cchCount1: INT,
        lpString2: *const u16,
        cchCount2: INT,
        bIgnoreCase: BOOL,
    ) -> INT;
    pub fn MultiByteToWideChar(
        CodePage: UINT,
        dwFlags: DWORD,
        lpMultiByteStr: *const BYTE,
        cbMultiByte: INT,
        lpWideCharStr: *mut u16,
        cchWideChar: INT,
    ) -> INT;
    pub fn GlobalAlloc(uFlags: UINT, dwBytes: usize) -> HGLOBAL;
    pub fn GlobalLock(hMem: HGLOBAL) -> *mut c_void;
    pub fn GlobalUnlock(hMem: HGLOBAL) -> BOOL;
    pub fn GlobalFree(hMem: HGLOBAL) -> HGLOBAL;
    pub fn SetThreadExecutionState(esFlags: DWORD) -> DWORD;
    pub fn GetLocalTime(lpSystemTime: *mut SYSTEMTIME);
    pub fn GetDiskFreeSpaceExW(
        lpDirectoryName: *const u16,
        lpFreeBytesAvailableToCaller: *mut u64,
        lpTotalNumberOfBytes: *mut u64,
        lpTotalNumberOfFreeBytes: *mut u64,
    ) -> BOOL;
}

/// Rust文字列をNUL終端UTF-16配列へ変換する。
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// NUL終端を除いてUTF-16スライスをStringへ変換する。
pub fn from_wide_nul(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// 96DPI基準の座標・サイズを現在DPIへスケーリングする。
pub fn scale_dpi(val: i32, dpi: u32) -> i32 {
    ((val as i64 * dpi as i64) + 48) as i32 / 96
}

/// Win32 `HANDLE` 用 RAII ガード。
pub struct HandleGuard(pub HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// GDI `HFONT` 用 RAII ガード。
pub struct FontGuard(pub HFONT);

impl Drop for FontGuard {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }
}

/// GDI `HBRUSH` 用 RAII ガード。
pub struct BrushGuard(pub HBRUSH);

impl Drop for BrushGuard {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                DeleteObject(self.0);
            }
        }
    }
}

/// `ExtractIconExW` で取得した大小アイコン用 RAII ガード。
pub struct IconPairGuard {
    pub large: HICON,
    pub small: HICON,
}

impl IconPairGuard {
    /// System32 の `shell32.dll` から標準フォルダ/同期系アイコンを取得する。
    pub fn load_app_icons() -> Self {
        let mut large: HICON = 0;
        let mut small: HICON = 0;
        let dll_path = get_system32_dir()
            .map(|d| d.join("shell32.dll"))
            .unwrap_or_else(|_| std::path::PathBuf::from("shell32.dll"));
        let dll_w = to_wide(&dll_path.to_string_lossy());
        unsafe {
            ExtractIconExW(dll_w.as_ptr(), 4, &raw mut large, &raw mut small, 1);
        }
        Self { large, small }
    }
}

impl Drop for IconPairGuard {
    fn drop(&mut self) {
        unsafe {
            if self.large != 0 {
                DestroyIcon(self.large);
            }
            if self.small != 0 && self.small != self.large {
                DestroyIcon(self.small);
            }
        }
    }
}

/// クリップボード開閉用 RAII ガード。
pub struct ClipboardGuard(bool);

impl ClipboardGuard {
    pub fn open(hwnd: HWND) -> Result<Self, String> {
        let ok = unsafe { OpenClipboard(hwnd) };
        if ok == 0 {
            return Err("クリップボードを開けませんでした。".to_string());
        }
        Ok(Self(true))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CloseClipboard();
            }
        }
    }
}

/// `GlobalAlloc` ハンドル用 RAII ガード（`SetClipboardData` 委譲時に `disarm`）。
pub struct GlobalMemGuard(pub HGLOBAL);

impl GlobalMemGuard {
    pub fn disarm(&mut self) -> HGLOBAL {
        let h = self.0;
        self.0 = 0;
        h
    }
}

impl Drop for GlobalMemGuard {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}

/// バックアップ実行中の自動スリープ抑止ガード。
pub struct ExecutionStateGuard;

impl ExecutionStateGuard {
    pub fn acquire() -> Self {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        }
        Self
    }
}

impl Drop for ExecutionStateGuard {
    fn drop(&mut self) {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

/// System32 ディレクトリ絶対パスを取得する。
pub fn get_system32_dir() -> Result<std::path::PathBuf, String> {
    let mut buf = [0u16; 4096];
    let len = unsafe { GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as UINT) };
    if len == 0 || (len as usize) >= buf.len() {
        return Err("System32ディレクトリの取得に失敗しました。".to_string());
    }
    Ok(std::path::PathBuf::from(String::from_utf16_lossy(
        &buf[..len as usize],
    )))
}

/// 自身の実行ファイル絶対パスを取得する。
pub fn get_self_exe_path() -> Result<std::path::PathBuf, String> {
    let mut buf = [0u16; 4096];
    let len = unsafe { GetModuleFileNameW(0, buf.as_mut_ptr(), buf.len() as DWORD) };
    if len == 0 || (len as usize) >= buf.len() {
        return Err("実行ファイルパスの取得に失敗しました。".to_string());
    }
    Ok(std::path::PathBuf::from(String::from_utf16_lossy(
        &buf[..len as usize],
    )))
}

/// 文字列をクリップボードへコピーする。
pub fn copy_text_to_clipboard(hwnd: HWND, text: &str) -> Result<(), String> {
    let _clip = ClipboardGuard::open(hwnd)?;
    if unsafe { EmptyClipboard() } == 0 {
        return Err("クリップボードの初期化に失敗しました。".to_string());
    }
    let wide = to_wide(text);
    let byte_len = wide.len() * std::mem::size_of::<u16>();
    let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) };
    if h_mem == 0 {
        return Err("クリップボード用メモリの確保に失敗しました。".to_string());
    }
    let mut guard = GlobalMemGuard(h_mem);
    let ptr = unsafe { GlobalLock(h_mem) } as *mut u16;
    if ptr.is_null() {
        return Err("クリップボード用メモリのロックに失敗しました。".to_string());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(h_mem);
    }
    let set_res = unsafe { SetClipboardData(CF_UNICODETEXT, h_mem) };
    if set_res == 0 {
        return Err("クリップボードへの設定に失敗しました。".to_string());
    }
    let _ = guard.disarm();
    Ok(())
}

/// ウィンドウのテキストを全量取得する。
pub fn get_window_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len as usize) + 1];
    let read = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as INT) };
    if read <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..read as usize])
}

/// ウィンドウにテキストを設定する。
pub fn set_window_text(hwnd: HWND, text: &str) {
    let w = to_wide(text);
    unsafe {
        SetWindowTextW(hwnd, w.as_ptr());
    }
}

/// ウィンドウのクラス名を取得する。
pub fn get_window_class_name(hwnd: HWND) -> String {
    if hwnd == 0 {
        return String::new();
    }
    let mut buf = [0u16; 64];
    let len = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as INT) };
    if len <= 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..len as usize])
}

/// Windows 11 以降のウィンドウに角丸（DWMWA_WINDOW_CORNER_PREFERENCE）を適用する。
pub fn apply_rounded_corners(hwnd: HWND) {
    let pref = DWMWCP_ROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const pref).cast(),
            std::mem::size_of::<DWORD>() as DWORD,
        );
    }
}

/// 単一行 `EDIT` コントロールへプレースホルダー（キューバナー）を設定する。
pub fn set_cue_banner(edit_hwnd: HWND, text: &str) {
    let w = to_wide(text);
    unsafe {
        SendMessageW(
            edit_hwnd,
            EM_SETCUEBANNER,
            TRUE as WPARAM,
            w.as_ptr() as LPARAM,
        );
    }
}

/// 現在ローカル時刻の `(YYYYMMDD_HHMMSS, YYYY-MM-DD HH:MM:SS)` タプルを返す。
pub fn format_local_timestamps() -> (String, String) {
    let mut st = SYSTEMTIME::default();
    unsafe {
        GetLocalTime(&raw mut st);
    }
    let stamp = format!(
        "{:04}{:02}{:02}_{:02}{:02}{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    );
    let readable = format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    );
    (stamp, readable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_dialog_icons_satisfy_is_intresource_on_64bit() {
        for (icon, expected) in [
            (TD_WARNING_ICON, 0xFFFFusize),
            (TD_ERROR_ICON, 0xFFFEusize),
            (TD_INFORMATION_ICON, 0xFFFDusize),
            (TD_SHIELD_ICON, 0xFFFCusize),
        ] {
            let raw = icon as usize;
            assert_eq!(raw, expected);
            assert_eq!(raw >> 16, 0);
        }
    }
}
