use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::domain::profile::AppConfig;

use super::system::{is_process_elevated, is_reparse_point};
use super::win32_ffi::{
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW, get_self_exe_path, to_wide,
};

const CONFIG_FILE_NAME: &str = "Robocopy-GUI_config.json";
const LOGS_DIR_NAME: &str = "logs";
const MAX_KEPT_LOG_FILES: usize = 50;

/// EXE同階層基準のポータブルファイルパス群。
#[derive(Debug, Clone)]
pub struct PortablePaths {
    pub exe_dir: PathBuf,
    pub config_path: PathBuf,
    pub logs_dir: PathBuf,
}

impl PortablePaths {
    pub fn resolve() -> Self {
        let exe_dir = get_self_exe_path()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        Self {
            config_path: exe_dir.join(CONFIG_FILE_NAME),
            logs_dir: exe_dir.join(LOGS_DIR_NAME),
            exe_dir,
        }
    }
}

/// 設定ファイルを読み込む。未存在ならデフォルトを返し、JSON破損時は警告メッセージを伴う。
pub fn load_config(path: &Path) -> (AppConfig, Option<String>) {
    if !path.exists() {
        return (AppConfig::default(), None);
    }
    match fs::read_to_string(path) {
        Ok(content) => {
            match serde_json::from_str::<AppConfig>(content.trim_start_matches('\u{FEFF}')) {
                Ok(mut cfg) => {
                    cfg.sanitize();
                    (cfg, None)
                }
                Err(e) => (
                    AppConfig::default(),
                    Some(format!(
                        "設定ファイルの解析に失敗したため既定設定で起動しました ({}): {e}",
                        path.display()
                    )),
                ),
            }
        }
        Err(e) => (
            AppConfig::default(),
            Some(format!(
                "設定ファイルを読み込めませんでした ({}): {e}",
                path.display()
            )),
        ),
    }
}

/// 設定ファイルを原子的リネーム（OneDrive共有ロック時は短リトライ＆フォールバック）で保存する。
pub fn save_config(path: &Path, config: &AppConfig) -> Result<(), String> {
    if is_process_elevated() && path.exists() && is_reparse_point(path) {
        return Err(
            "セキュリティ保護のため、リパースポイント（シンボリックリンク）上の設定ファイルへの管理者書き込みをブロックしました。"
                .to_string(),
        );
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("設定のJSON変換に失敗しました: {e}"))?;

    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json.as_bytes()).map_err(|e| {
        format!("設定ファイルを保存できませんでした（読み取り専用フォルダ等の可能性）: {e}")
    })?;

    let tmp_w = to_wide(&tmp_path.to_string_lossy());
    let dst_w = to_wide(&path.to_string_lossy());
    for attempt in 0..3 {
        let ok = unsafe {
            MoveFileExW(
                tmp_w.as_ptr(),
                dst_w.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if ok != 0 {
            return Ok(());
        }
        if attempt < 2 {
            thread::sleep(Duration::from_millis(50));
        }
    }

    // OneDrive等による一時ロック時のフォールバック直接上書き
    let direct_res = fs::write(path, json.as_bytes());
    let _ = fs::remove_file(&tmp_path);
    direct_res.map_err(|e| format!("設定ファイルの書き込みに失敗しました: {e}"))
}

/// UAC昇格引き継ぎ用の一時設定ファイルを `%TEMP%` へ出力する。
pub fn write_handoff_config(config: &AppConfig) -> Result<PathBuf, String> {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("Robocopy-GUI_handoff_{pid}.json"));
    if path.exists() && is_reparse_point(&path) {
        return Err("引き継ぎ一時ファイルパスが不正（リパースポイント）です。".to_string());
    }
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("引き継ぎ設定の変換に失敗しました: {e}"))?;
    fs::write(&path, json.as_bytes())
        .map_err(|e| format!("引き継ぎ一時ファイルの保存に失敗しました: {e}"))?;
    Ok(path)
}

/// `--handoff <path>` 引数で指定された一時設定を読み込み、読み込み後に削除する。
pub fn load_and_cleanup_handoff(path: &Path) -> Option<AppConfig> {
    let content = fs::read_to_string(path).ok()?;
    let _ = fs::remove_file(path);
    let mut cfg = serde_json::from_str::<AppConfig>(content.trim_start_matches('\u{FEFF}')).ok()?;
    cfg.sanitize();
    Some(cfg)
}

/// 実行ログを `logs/` フォルダへ UTF-8 保存し、古いログを最大50件に維持する。
pub fn write_execution_log(
    logs_dir: &Path,
    stamp: &str,
    lines: &[String],
) -> Result<PathBuf, String> {
    if is_process_elevated() && logs_dir.exists() && is_reparse_point(logs_dir) {
        return Err(
            "セキュリティ保護のため、リパースポイント上の logs フォルダへの管理者書き込みをブロックしました。"
                .to_string(),
        );
    }
    fs::create_dir_all(logs_dir)
        .map_err(|e| format!("logs フォルダを作成できませんでした: {e}"))?;

    let file_path = logs_dir.join(format!("robocopy_{stamp}.log"));
    let mut content = String::new();
    for line in lines {
        content.push_str(line);
        content.push_str("\r\n");
    }
    fs::write(&file_path, content.as_bytes())
        .map_err(|e| format!("ログファイルの保存に失敗しました: {e}"))?;

    prune_old_logs(logs_dir, MAX_KEPT_LOG_FILES);
    Ok(file_path)
}

fn prune_old_logs(logs_dir: &Path, max_files: usize) {
    let Ok(entries) = fs::read_dir(logs_dir) else {
        return;
    };
    let mut log_files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("log") {
                let mtime = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                Some((mtime, p))
            } else {
                None
            }
        })
        .collect();

    if log_files.len() <= max_files {
        return;
    }
    log_files.sort_by_key(|(mtime, _)| *mtime);
    let remove_count = log_files.len() - max_files;
    for (_, p) in log_files.into_iter().take(remove_count) {
        let _ = fs::remove_file(p);
    }
}

/// `.bat` スクリプトを UTF-8 (BOMなし・`chcp 65001`入り) かつ CRLF で保存する。
pub fn export_bat_file(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content.as_bytes())
        .map_err(|e| format!("バッチファイルの保存に失敗しました: {e}"))
}
