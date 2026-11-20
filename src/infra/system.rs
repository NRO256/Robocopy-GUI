use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use crate::domain::validator::{PathInspector, is_root_or_known_critical, normalize_for_compare};

use super::win32_ffi::*;

/// Win32 API を用いた実体パス解決・アクセス判定・UAC昇格・シャットダウン制御。
pub struct Win32PathInspector;

impl Win32PathInspector {
    /// マップドドライブ（例: `Z:`）がネットワーク共有へ接続されていれば UNC パスを返す。
    pub fn resolve_mapped_drive_unc(drive_prefix: &str) -> Option<String> {
        if drive_prefix.len() != 2 || !drive_prefix.as_bytes()[0].is_ascii_alphabetic() {
            return None;
        }
        let local_w = to_wide(drive_prefix);
        let mut remote_buf = vec![0u16; 1024];
        let mut len = remote_buf.len() as DWORD;
        let rc =
            unsafe { WNetGetConnectionW(local_w.as_ptr(), remote_buf.as_mut_ptr(), &raw mut len) };
        if rc == 0 {
            let unc = from_wide_nul(&remote_buf);
            if !unc.is_empty() {
                return Some(unc);
            }
        }
        None
    }

    fn expand_mapped_drive_if_needed(norm: &str) -> String {
        let bytes = norm.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let drive = &norm[..2];
            if let Some(unc) = Self::resolve_mapped_drive_unc(drive) {
                let tail = norm[2..].trim_start_matches('\\');
                if tail.is_empty() {
                    return unc;
                }
                return format!("{}\\{}", unc.trim_end_matches('\\'), tail);
            }
        }
        norm.to_string()
    }

    fn get_final_path_win32(existing: &Path) -> Option<String> {
        let wide = to_wide(&existing.to_string_lossy());
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                null_mut(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                0,
            )
        };
        if handle == INVALID_HANDLE_VALUE || handle == 0 {
            return None;
        }
        let _guard = HandleGuard(handle);
        let mut buf = vec![0u16; 32768];
        let len =
            unsafe { GetFinalPathNameByHandleW(handle, buf.as_mut_ptr(), buf.len() as DWORD, 0) };
        if len == 0 || (len as usize) >= buf.len() {
            return None;
        }
        let raw = String::from_utf16_lossy(&buf[..len as usize]);
        Some(normalize_for_compare(&raw))
    }

    fn compare_ordinal_ci(a: &str, b: &str) -> bool {
        let wa = to_wide(a);
        let wb = to_wide(b);
        let res = unsafe {
            CompareStringOrdinal(
                wa.as_ptr(),
                (wa.len().saturating_sub(1)) as INT,
                wb.as_ptr(),
                (wb.len().saturating_sub(1)) as INT,
                TRUE,
            )
        };
        res == CSTR_EQUAL
    }
}

impl PathInspector for Win32PathInspector {
    fn dir_exists(&self, path: &str) -> bool {
        let p = Path::new(path);
        p.exists() && p.is_dir()
    }

    fn is_dir_empty(&self, path: &str) -> Option<bool> {
        let mut entries = std::fs::read_dir(path).ok()?;
        Some(entries.next().is_none())
    }

    fn resolve_canonical_path(&self, path: &str) -> Result<String, String> {
        let cleaned = normalize_for_compare(path);
        if cleaned.is_empty() {
            return Ok(String::new());
        }

        // 未存在のコピー先にも対応するため、存在する最深親ディレクトリまで遡って実体解決する。
        let mut current = PathBuf::from(&cleaned);
        let mut tail_parts: Vec<String> = Vec::new();
        let mut resolved_base: Option<String> = None;

        loop {
            if current.exists()
                && let Some(final_p) = Self::get_final_path_win32(&current)
            {
                resolved_base = Some(final_p);
                break;
            }
            let Some(file_name) = current.file_name() else {
                break;
            };
            tail_parts.push(file_name.to_string_lossy().to_string());
            if !current.pop() {
                break;
            }
        }

        let base_candidate = resolved_base.unwrap_or(cleaned);
        let mut base = Self::expand_mapped_drive_if_needed(&base_candidate);
        if tail_parts.is_empty() {
            return Ok(normalize_for_compare(&base));
        }

        for part in tail_parts.into_iter().rev() {
            if !base.ends_with('\\') {
                base.push('\\');
            }
            base.push_str(&part);
        }
        Ok(normalize_for_compare(&base))
    }

    fn paths_equal_ci(&self, a: &str, b: &str) -> bool {
        let na = normalize_for_compare(a);
        let nb = normalize_for_compare(b);
        Self::compare_ordinal_ci(&na, &nb)
    }

    fn is_ancestor_ci(&self, ancestor: &str, child: &str) -> bool {
        let anc = normalize_for_compare(ancestor);
        let ch = normalize_for_compare(child);
        if anc.is_empty() || ch.is_empty() {
            return false;
        }
        let prefix_utf16: Vec<u16> = if anc.ends_with('\\') {
            anc.encode_utf16().collect()
        } else {
            anc.encode_utf16()
                .chain(std::iter::once(b'\\' as u16))
                .collect()
        };
        let ch_utf16: Vec<u16> = ch.encode_utf16().take(prefix_utf16.len() + 1).collect();
        if ch_utf16.len() <= prefix_utf16.len() {
            return false;
        }
        let res = unsafe {
            CompareStringOrdinal(
                ch_utf16.as_ptr(),
                prefix_utf16.len() as INT,
                prefix_utf16.as_ptr(),
                prefix_utf16.len() as INT,
                TRUE,
            )
        };
        res == CSTR_EQUAL
    }

    fn is_critical_system_dir(&self, canonical_path: &str) -> bool {
        is_root_or_known_critical(canonical_path)
    }
}

/// 現在のプロセスが管理者権限（UAC昇格済み）で動作しているか判定する。
pub fn is_process_elevated() -> bool {
    let mut token: HANDLE = 0;
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) };
    if ok == 0 || token == 0 {
        return false;
    }
    let _guard = HandleGuard(token);
    let mut elevation = TOKEN_ELEVATION::default();
    let mut ret_len: DWORD = 0;
    let res = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&raw mut elevation).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as DWORD,
            &raw mut ret_len,
        )
    };
    res != 0 && elevation.TokenIsElevated != 0
}

/// ファイルまたはディレクトリがリパースポイント（ジャンクション/シンボリックリンク）か判定する。
pub fn is_reparse_point(path: &Path) -> bool {
    let w = to_wide(&path.to_string_lossy());
    let attrs = unsafe { GetFileAttributesW(w.as_ptr()) };
    attrs != INVALID_FILE_ATTRIBUTES && (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0
}

/// 指定パス群に含まれるネットワークマップドドライブ（例: `Z:` -> `\\NAS\Share`）を検出する。
pub fn detect_mapped_drives(paths: &[&str]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for p in paths {
        let t = p.trim();
        let bytes = t.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            let drive = t[..2].to_ascii_uppercase();
            if found.iter().any(|(d, _)| d == &drive) {
                continue;
            }
            if let Some(unc) = Win32PathInspector::resolve_mapped_drive_unc(&drive) {
                found.push((drive, unc));
            }
        }
    }
    found
}

/// UAC「管理者として再起動」を実行する。
pub fn restart_as_admin(hwnd: HWND, handoff_path: Option<&Path>) -> Result<(), String> {
    let exe_path = get_self_exe_path()?;
    let exe_str = exe_path.to_string_lossy().to_string();
    let exe_w = to_wide(&exe_str);
    let verb_w = to_wide("runas");
    let params_str = match handoff_path {
        Some(p) => format!("--restarted --handoff \"{}\"", p.to_string_lossy()),
        None => "--restarted".to_string(),
    };
    let params_w = Some(to_wide(&params_str));
    let work_dir_w = exe_path.parent().map(|d| to_wide(&d.to_string_lossy()));

    let hinst = unsafe {
        ShellExecuteW(
            hwnd,
            verb_w.as_ptr(),
            exe_w.as_ptr(),
            params_w.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            work_dir_w.as_ref().map_or(std::ptr::null(), |v| v.as_ptr()),
            SW_SHOW,
        )
    };
    if hinst <= 32 {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "管理者昇格の起動がキャンセルされたか失敗しました (ShellExecute code={hinst}, Win32={err})。"
        ));
    }
    Ok(())
}

/// System32 配下の `shutdown.exe /s /t 0` を実行してPCをシャットダウンする（`/f` は使用しない）。
pub fn execute_system_shutdown() -> Result<(), String> {
    let sys32 = get_system32_dir()?;
    let shutdown_exe = sys32.join("shutdown.exe");
    let status = std::process::Command::new(&shutdown_exe)
        .args(["/s", "/t", "0"])
        .spawn()
        .map_err(|e| format!("shutdown.exe の起動に失敗しました: {e}"))?;
    let _ = status;
    Ok(())
}

/// 指定パス（または存在する最深親ディレクトリ）のディスク空き容量と総容量 `(free_bytes, total_bytes)` を取得する。
pub fn query_disk_space_summary(raw_path: &str) -> Option<(u64, u64)> {
    let norm = normalize_for_compare(raw_path);
    if norm.is_empty() {
        return None;
    }
    let mut cur = PathBuf::from(&norm);
    loop {
        if cur.exists() {
            let target = if cur.is_dir() {
                cur
            } else {
                cur.parent()?.to_path_buf()
            };
            let w = to_wide(&target.to_string_lossy());
            let mut free_avail: u64 = 0;
            let mut total: u64 = 0;
            let mut total_free: u64 = 0;
            let ok = unsafe {
                GetDiskFreeSpaceExW(
                    w.as_ptr(),
                    &raw mut free_avail,
                    &raw mut total,
                    &raw mut total_free,
                )
            };
            if ok != 0 && total > 0 {
                return Some((free_avail, total));
            }
            return None;
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// バイト数を人間が読みやすい単位文字列へ整形する。
pub fn format_bytes_human(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.1} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// コピー元・コピー先パスの実在確認と空き容量サマリー文字列を返す（第2要素は警告表示フラグ）。
pub fn describe_paths_status(source: &str, destination: &str) -> (String, bool) {
    let src_trim = source.trim();
    let dst_trim = destination.trim();
    if src_trim.is_empty() && dst_trim.is_empty() {
        return (
            "ヒント: エクスプローラーからフォルダをドラッグ＆ドロップして指定できます".to_string(),
            false,
        );
    }

    let (src_msg, src_warn) = if src_trim.is_empty() {
        ("コピー元: 未指定".to_string(), false)
    } else {
        let norm = normalize_for_compare(src_trim);
        let p = Path::new(&norm);
        if p.exists() && p.is_dir() {
            ("コピー元: 確認OK".to_string(), false)
        } else if p.exists() {
            ("コピー元: フォルダではありません".to_string(), true)
        } else {
            ("コピー元: 見つかりません".to_string(), true)
        }
    };

    let (dst_msg, dst_warn) = if dst_trim.is_empty() {
        ("コピー先: 未指定".to_string(), false)
    } else {
        let norm = normalize_for_compare(dst_trim);
        let p = Path::new(&norm);
        let (state, state_warn) = if p.exists() && p.is_dir() {
            ("既存フォルダ", false)
        } else if p.exists() {
            ("フォルダと同名のファイルあり", true)
        } else {
            ("新規作成予定", false)
        };
        if let Some((free, total)) = query_disk_space_summary(dst_trim) {
            (
                format!(
                    "コピー先: {state} (空き {} / {})",
                    format_bytes_human(free),
                    format_bytes_human(total)
                ),
                state_warn,
            )
        } else {
            (format!("コピー先: {state} (容量取得不可)"), true)
        }
    };

    (format!("{src_msg} | {dst_msg}"), src_warn || dst_warn)
}

/// 指定パス（未存在時は存在する最深親ディレクトリ）をエクスプローラーで開く。
pub fn open_folder_or_parent_in_explorer(hwnd: HWND, raw_path: &str) -> Result<(), String> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err("パスが入力されていません。".to_string());
    }
    let norm = normalize_for_compare(trimmed);
    let mut cur = PathBuf::from(&norm);
    while !cur.exists() {
        if !cur.pop() {
            return Err(format!("開けるフォルダが見つかりません: {norm}"));
        }
    }
    let target = if cur.is_dir() {
        cur
    } else {
        cur.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| format!("親フォルダが見つかりません: {norm}"))?
    };

    let op_w = to_wide("open");
    let path_w = to_wide(&target.to_string_lossy());
    let res = unsafe {
        ShellExecuteW(
            hwnd,
            op_w.as_ptr(),
            path_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOW,
        )
    };
    if res <= 32 {
        return Err(format!(
            "エクスプローラーでフォルダを開けませんでした: {}",
            target.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn format_bytes_human_formats_all_units() {
        assert_eq!(format_bytes_human(512 * 1024), "512 KB");
        assert_eq!(format_bytes_human(2 * 1024 * 1024), "2.0 MB");
        assert_eq!(format_bytes_human(5 * 1024 * 1024 * 1024), "5.0 GB");
        assert_eq!(format_bytes_human(3 * 1024_u64.pow(4) / 2), "1.50 TB");
    }

    #[test]
    fn describe_paths_status_reports_empty_missing_and_valid_paths() {
        let (empty_msg, empty_warn) = describe_paths_status("", "");
        assert!(!empty_warn);
        assert!(empty_msg.contains("ドラッグ＆ドロップ"));

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("rc_sys_test_{nanos}"));
        let src = base.join("src");
        let dst = base.join("dst_new");
        std::fs::create_dir_all(&src).unwrap();

        let (ok_msg, ok_warn) =
            describe_paths_status(&src.to_string_lossy(), &dst.to_string_lossy());
        assert!(!ok_warn);
        assert!(ok_msg.contains("コピー元: 確認OK"));
        assert!(ok_msg.contains("コピー先: 新規作成予定"));
        assert!(ok_msg.contains("空き "));

        let missing = base.join("missing_dir");
        let (miss_msg, miss_warn) = describe_paths_status(&missing.to_string_lossy(), "");
        assert!(miss_warn);
        assert!(miss_msg.contains("コピー元: 見つかりません"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
