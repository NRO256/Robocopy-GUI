use std::path::Path;

use super::command::{CommandSpec, build_command_spec};
use super::profile::BackupProfile;
use super::queue::ExecutionPlan;

/// 検証エラー発生時のフォーカス誘導先。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusField {
    Source,
    Destination,
    ExtraArgs,
    QueueList,
    BackupMode,
}

/// 検証結果（ブロックエラーまたは警告）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub errors: Vec<(FocusField, String)>,
    pub warnings: Vec<String>,
    pub destructive_summaries: Vec<String>,
    pub command_previews: Vec<String>,
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// OS依存のパス解決・属性検査を抽象化するトレイト（純粋単体テスト可能にする）。
pub trait PathInspector {
    /// 指定パスが既存ディレクトリか確認する。
    fn dir_exists(&self, path: &str) -> bool;
    /// 既存ディレクトリのエントリが空（0件）かどうか。未存在や読取不可は `None`。
    fn is_dir_empty(&self, path: &str) -> Option<bool>;
    /// 最深既存親ディレクトリの実体解決とマップドドライブUNC解決を行い比較用正規化パスを返す。
    fn resolve_canonical_path(&self, path: &str) -> Result<String, String>;
    /// 大文字小文字を区別せず2つの正規化パスが一致するか判定する。
    fn paths_equal_ci(&self, a: &str, b: &str) -> bool;
    /// `ancestor` が `child` の親ディレクトリ（真の上位階層）かどうか大文字小文字無視で判定する。
    fn is_ancestor_ci(&self, ancestor: &str, child: &str) -> bool;
    /// `/MIR`・`/MOVE` 等の破壊対象として禁止すべき重要システムフォルダか判定する。
    fn is_critical_system_dir(&self, canonical_path: &str) -> bool;
}

/// 標準Rustパス比較ベースのテスト/フォールバック実装（本番では `Win32PathInspector` を使用）。
pub struct FallbackPathInspector;

impl PathInspector for FallbackPathInspector {
    fn dir_exists(&self, path: &str) -> bool {
        let p = Path::new(path);
        p.exists() && p.is_dir()
    }

    fn is_dir_empty(&self, path: &str) -> Option<bool> {
        let mut entries = std::fs::read_dir(path).ok()?;
        Some(entries.next().is_none())
    }

    fn resolve_canonical_path(&self, path: &str) -> Result<String, String> {
        Ok(normalize_for_compare(path))
    }

    fn paths_equal_ci(&self, a: &str, b: &str) -> bool {
        normalize_for_compare(a).eq_ignore_ascii_case(&normalize_for_compare(b))
    }

    fn is_ancestor_ci(&self, ancestor: &str, child: &str) -> bool {
        let anc = normalize_for_compare(ancestor);
        let ch = normalize_for_compare(child);
        if anc.eq_ignore_ascii_case(&ch) {
            return false;
        }
        let anc_lower = anc.to_lowercase();
        let ch_lower = ch.to_lowercase();
        if anc_lower.ends_with('\\') {
            ch_lower.starts_with(&anc_lower)
        } else {
            ch_lower.starts_with(&format!("{anc_lower}\\"))
        }
    }

    fn is_critical_system_dir(&self, canonical_path: &str) -> bool {
        is_root_or_known_critical(&normalize_for_compare(canonical_path))
    }
}

/// 比較用の簡易正規化（`\\?\` プレフィックス除去、末尾 `\.` や `\` 整理）。
pub fn normalize_for_compare(raw: &str) -> String {
    let mut s = raw.trim().replace('/', "\\");
    if let Some(stripped) = s.strip_prefix("\\\\?\\UNC\\") {
        s = format!("\\\\{stripped}");
    } else if let Some(stripped) = s.strip_prefix("\\\\?\\") {
        s = stripped.to_string();
    }
    if s.ends_with("\\.") && s.len() == 4 && s.as_bytes()[1] == b':' {
        s.pop(); // "D:\." -> "D:\"
    }
    let bytes = s.as_bytes();
    let is_drive_root =
        bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":\\";
    if !is_drive_root {
        s = s.trim_end_matches('\\').to_string();
    }
    s
}

/// ドライブ直下、UNC共有ルート直下、または既知OS重要フォルダかどうか判定する。
pub fn is_root_or_known_critical(canonical_path: &str) -> bool {
    let norm = normalize_for_compare(canonical_path);
    let bytes = norm.as_bytes();
    // ドライブルート: "C:\" or "C:"
    if (bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || (bytes.len() == 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == b":\\")
    {
        return true;
    }

    // UNC共有ルート: "\\server\share"（スラッシュ2階層のみ）
    if let Some(unc) = norm.strip_prefix("\\\\") {
        let segs: Vec<&str> = unc.split('\\').filter(|p| !p.is_empty()).collect();
        if segs.len() <= 2 {
            return true;
        }
    }

    let lower = norm.to_lowercase();
    for env_key in [
        "SystemRoot",
        "WINDIR",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "USERPROFILE",
    ] {
        if let Ok(val) = std::env::var(env_key) {
            let env_norm = normalize_for_compare(&val).to_lowercase();
            if !env_norm.is_empty() && lower == env_norm {
                return true;
            }
        }
    }
    // Users ルート（例: C:\Users）
    if let Ok(sys_drive) = std::env::var("SystemDrive") {
        let users_root = format!("{}\\users", sys_drive.trim_end_matches('\\')).to_lowercase();
        if lower == users_root {
            return true;
        }
    }
    false
}

/// 実行計画全体の事前安全検証を実施する。
pub fn validate_execution_plan(
    plan: &ExecutionPlan,
    inspector: &dyn PathInspector,
    is_elevated: bool,
) -> ValidationResult {
    let mut result = ValidationResult {
        errors: Vec::new(),
        warnings: Vec::new(),
        destructive_summaries: Vec::new(),
        command_previews: Vec::new(),
    };

    if plan.jobs.is_empty() {
        result.errors.push((
            FocusField::QueueList,
            "実行対象のジョブがありません。有効なキュー項目を1件以上チェックしてください。"
                .to_string(),
        ));
        return result;
    }

    for (idx, job) in plan.jobs.iter().enumerate() {
        let job_label = if plan.is_queue_mode {
            format!("[ジョブ {}: {}] ", idx + 1, job.display_name)
        } else {
            String::new()
        };

        let spec =
            match validate_single_profile(&job.profile, plan.is_dry_run, inspector, is_elevated) {
                Ok((spec, warns)) => {
                    result
                        .warnings
                        .extend(warns.into_iter().map(|w| format!("{job_label}{w}")));
                    spec
                }
                Err((field, msg)) => {
                    result.errors.push((field, format!("{job_label}{msg}")));
                    continue;
                }
            };

        if spec.has_destructive_flags && !plan.is_dry_run {
            let mode_note = if spec.has_mirror_or_purge {
                "/MIR・/PURGE（コピー先の余分ファイル削除）"
            } else {
                "/MOVE・/MOV（コピー元ファイル削除）"
            };
            result.destructive_summaries.push(format!(
                "{job_label}{mode_note}: {} -> {}",
                spec.normalized_source, spec.normalized_dest
            ));
        }
        result
            .command_previews
            .push(format!("{job_label}{}", spec.display_command));
    }

    result
}

/// 1プロファイル分の詳細検証。
pub fn validate_single_profile(
    profile: &BackupProfile,
    is_dry_run: bool,
    inspector: &dyn PathInspector,
    is_elevated: bool,
) -> Result<(CommandSpec, Vec<String>), (FocusField, String)> {
    if profile.source.trim().is_empty() {
        return Err((
            FocusField::Source,
            "コピー元フォルダが指定されていません。".to_string(),
        ));
    }
    if profile.destination.trim().is_empty() {
        return Err((
            FocusField::Destination,
            "コピー先フォルダが指定されていません。".to_string(),
        ));
    }

    let spec = build_command_spec(profile, is_dry_run).map_err(|e| {
        let field = if e.is_extra_args_error {
            FocusField::ExtraArgs
        } else {
            FocusField::Source
        };
        (field, e.message)
    })?;

    if !inspector.dir_exists(&spec.normalized_source) {
        return Err((
            FocusField::Source,
            format!(
                "コピー元フォルダが存在しないかフォルダではありません: {}",
                spec.normalized_source
            ),
        ));
    }

    let src_canon = inspector
        .resolve_canonical_path(&spec.normalized_source)
        .unwrap_or_else(|_| normalize_for_compare(&spec.normalized_source));
    let dst_canon = inspector
        .resolve_canonical_path(&spec.normalized_dest)
        .unwrap_or_else(|_| normalize_for_compare(&spec.normalized_dest));

    if inspector.paths_equal_ci(&src_canon, &dst_canon) {
        return Err((
            FocusField::Destination,
            "コピー元とコピー先に同一フォルダを指定することはできません。".to_string(),
        ));
    }
    if inspector.is_ancestor_ci(&src_canon, &dst_canon) {
        return Err((
            FocusField::Destination,
            "コピー先がコピー元フォルダの配下にあるため、無限再帰防止のため実行できません。"
                .to_string(),
        ));
    }
    if inspector.is_ancestor_ci(&dst_canon, &src_canon) {
        return Err((
            FocusField::Destination,
            "コピー元がコピー先フォルダの配下にあるため、安全上実行できません。".to_string(),
        ));
    }

    if spec.has_destructive_flags {
        if inspector.is_critical_system_dir(&dst_canon) {
            return Err((
                FocusField::Destination,
                format!(
                    "ドライブ直下・共有ルート・システム重要フォルダへの破壊的操作 (/MIR /MOVE /PURGE) は禁止されています: {}",
                    spec.normalized_dest
                ),
            ));
        }
        if spec.has_mirror_or_purge && inspector.is_dir_empty(&spec.normalized_source) == Some(true)
        {
            return Err((
                FocusField::Source,
                format!(
                    "コピー元フォルダが空（0件）の状態で /MIR または /PURGE を実行するとコピー先が全消去されるためブロックしました: {}",
                    spec.normalized_source
                ),
            ));
        }
    }

    let mut warnings = Vec::new();
    if spec.uses_backup_only_privilege && !is_elevated {
        return Err((
            FocusField::BackupMode,
            "/B（バックアップ権限モード）を使用するには管理者権限が必要です。メニューの「ツール -> 管理者として再起動」から昇格してください。".to_string(),
        ));
    }
    if spec.uses_zb_privilege && !is_elevated {
        warnings.push(
            "/ZB 指定ですが現在通常権限のため、アクセス拒否箇所では再開可能モードへフォールバックします（必要時はツール->管理者として再起動）。".to_string(),
        );
    }

    Ok((spec, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::BackupPrivilegeMode;
    use crate::domain::profile::CopyMode;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_pair(suffix: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("rc_val_test_{suffix}_{nanos}"));
        let src = base.join("src");
        let dst = base.join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("sample.txt"), b"ok").unwrap();
        (src, dst)
    }

    #[test]
    fn rejects_same_and_parent_child_paths_case_insensitively() {
        let (src, _) = make_temp_pair("same");
        let src_str = src.to_string_lossy().to_string();
        let inspector = FallbackPathInspector;

        let same_profile = BackupProfile {
            source: src_str.clone(),
            destination: src_str.to_lowercase(),
            ..Default::default()
        };
        assert!(validate_single_profile(&same_profile, false, &inspector, false).is_err());

        let child_dst = format!("{}\\sub", src_str);
        let child_profile = BackupProfile {
            source: src_str.clone(),
            destination: child_dst,
            ..Default::default()
        };
        assert!(validate_single_profile(&child_profile, false, &inspector, false).is_err());

        let _ = std::fs::remove_dir_all(src.parent().unwrap());
    }

    #[test]
    fn rejects_mir_on_drive_root_unc_root_and_empty_source() {
        let (src, dst) = make_temp_pair("mir");
        let empty_src = src.parent().unwrap().join("empty_src");
        std::fs::create_dir_all(&empty_src).unwrap();
        let inspector = FallbackPathInspector;

        // Drive root
        let p_drive = BackupProfile {
            source: src.to_string_lossy().to_string(),
            destination: "D:\\".into(),
            copy_mode: CopyMode::Mirror,
            ..Default::default()
        };
        assert!(validate_single_profile(&p_drive, false, &inspector, false).is_err());

        // UNC share root
        let p_unc = BackupProfile {
            source: src.to_string_lossy().to_string(),
            destination: "\\\\NAS\\Share".into(),
            copy_mode: CopyMode::Mirror,
            ..Default::default()
        };
        assert!(validate_single_profile(&p_unc, false, &inspector, false).is_err());

        // Empty source with /MIR
        let p_empty = BackupProfile {
            source: empty_src.to_string_lossy().to_string(),
            destination: dst.to_string_lossy().to_string(),
            copy_mode: CopyMode::Mirror,
            ..Default::default()
        };
        assert!(validate_single_profile(&p_empty, false, &inspector, false).is_err());

        let _ = std::fs::remove_dir_all(src.parent().unwrap());
    }

    #[test]
    fn rejects_backup_only_when_not_elevated() {
        let (src, dst) = make_temp_pair("priv");
        let inspector = FallbackPathInspector;
        let p = BackupProfile {
            source: src.to_string_lossy().to_string(),
            destination: dst.to_string_lossy().to_string(),
            backup_mode: BackupPrivilegeMode::BackupOnly,
            ..Default::default()
        };
        assert!(validate_single_profile(&p, false, &inspector, false).is_err());
        assert!(validate_single_profile(&p, false, &inspector, true).is_ok());

        let _ = std::fs::remove_dir_all(src.parent().unwrap());
    }
}
