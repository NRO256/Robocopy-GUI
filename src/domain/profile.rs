use serde::{Deserialize, Serialize};

use super::queue::QueueItem;

/// 設定ファイルの現行スキーマバージョン。
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// robocopy のコピー動作モード。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CopyMode {
    /// `/E`: サブディレクトリを空も含めてコピー（既定推奨）。
    #[default]
    IncludeEmpty,
    /// `/S`: サブディレクトリをコピー（空ディレクトリは除外）。
    ExcludeEmpty,
    /// `/MIR`: ミラーリング（`/E` + `/PURGE`、コピー先余分を削除）。
    Mirror,
    /// `/MOVE`: コピー後にコピー元ファイルを削除。
    Move,
}

impl CopyMode {
    /// UI表示用の短いラベルを返す。
    pub fn short_label(self) -> &'static str {
        match self {
            Self::IncludeEmpty => "差分コピー (/E)",
            Self::ExcludeEmpty => "差分コピー (/S)",
            Self::Mirror => "ミラーリング (/MIR)",
            Self::Move => "移動 (/MOVE)",
        }
    }

    /// 破壊的変更（コピー先削除またはコピー元削除）を伴うモードかどうか。
    pub fn is_destructive(self) -> bool {
        matches!(self, Self::Mirror | Self::Move)
    }

    /// robocopy コマンド引数を返す。
    pub fn flag(self) -> &'static str {
        match self {
            Self::IncludeEmpty => "/E",
            Self::ExcludeEmpty => "/S",
            Self::Mirror => "/MIR",
            Self::Move => "/MOVE",
        }
    }
}

/// バックアップ権限モード（`/B` または `/ZB`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackupPrivilegeMode {
    /// 指定なし。
    #[default]
    None,
    /// `/ZB`: 再開可能モード、拒否時はバックアップ権限モードにフォールバック。
    RestartableFallback,
    /// `/B`: バックアップ権限モード（管理者権限必須）。
    BackupOnly,
}

impl BackupPrivilegeMode {
    /// コンボボックス表示名。
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "指定なし（標準権限）",
            Self::RestartableFallback => "/ZB（再開＋管理者権限フォールバック）",
            Self::BackupOnly => "/B（バックアップ権限のみ・管理者必須）",
        }
    }

    /// コンボボックスインデックスから変換する。
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::RestartableFallback,
            2 => Self::BackupOnly,
            _ => Self::None,
        }
    }

    /// コンボボックスインデックスへ変換する。
    pub fn to_index(self) -> usize {
        match self {
            Self::None => 0,
            Self::RestartableFallback => 1,
            Self::BackupOnly => 2,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_mt_count() -> u32 {
    8
}

fn default_retry_count() -> u32 {
    3
}

fn default_wait_seconds() -> u32 {
    5
}

fn default_profile_name() -> String {
    "既定".to_string()
}

/// 1件のバックアップ設定プロファイル。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackupProfile {
    pub name: String,
    pub source: String,
    pub destination: String,
    pub copy_mode: CopyMode,
    pub exclude_junctions: bool,
    pub fat_time_tolerance: bool,
    pub restartable: bool,
    pub backup_mode: BackupPrivilegeMode,
    pub copy_security: bool,
    pub exclude_dirs: String,
    pub exclude_files: String,
    pub exclude_hidden: bool,
    pub exclude_system: bool,
    pub multithread_enabled: bool,
    pub multithread_count: u32,
    pub retry_count: u32,
    pub wait_seconds: u32,
    pub auto_save_log: bool,
    pub no_progress: bool,
    pub verbose: bool,
    pub extra_args: String,
}

impl Default for BackupProfile {
    fn default() -> Self {
        Self {
            name: default_profile_name(),
            source: String::new(),
            destination: String::new(),
            copy_mode: CopyMode::IncludeEmpty,
            exclude_junctions: default_true(),
            fat_time_tolerance: false,
            restartable: false,
            backup_mode: BackupPrivilegeMode::None,
            copy_security: false,
            exclude_dirs: String::new(),
            exclude_files: String::new(),
            exclude_hidden: false,
            exclude_system: false,
            multithread_enabled: true,
            multithread_count: default_mt_count(),
            retry_count: default_retry_count(),
            wait_seconds: default_wait_seconds(),
            auto_save_log: default_true(),
            no_progress: default_true(),
            verbose: false,
            extra_args: String::new(),
        }
    }
}

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// ポータブル設定ファイル（`Robocopy-GUI_config.json`）全体モデル。
/// 完了後シャットダウン設定は安全のため永続化対象から構造的に除外している。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub schema_version: u32,
    pub selected_preset_index: usize,
    pub profiles: Vec<BackupProfile>,
    pub queue: Vec<QueueItem>,
    pub continue_queue_on_error: bool,
    pub last_form: BackupProfile,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_profile = BackupProfile::default();
        let dev_exclude_profile = BackupProfile {
            name: "開発フォルダ除外差分".to_string(),
            exclude_dirs: ".git;node_modules;target;$RECYCLE.BIN".to_string(),
            exclude_files: "*.tmp;*.bak;Thumbs.db".to_string(),
            ..BackupProfile::default()
        };
        let nas_fft_profile = BackupProfile {
            name: "外付けHDD/NAS差分 (/FFT)".to_string(),
            fat_time_tolerance: true,
            multithread_count: 4,
            exclude_dirs: "$RECYCLE.BIN;System Volume Information".to_string(),
            ..BackupProfile::default()
        };
        Self {
            schema_version: default_schema_version(),
            selected_preset_index: 0,
            profiles: vec![
                default_profile.clone(),
                dev_exclude_profile,
                nas_fft_profile,
            ],
            queue: Vec::new(),
            continue_queue_on_error: true,
            last_form: default_profile,
        }
    }
}

impl AppConfig {
    /// ロード後の整合性を補正する（プリセット空回避・クランプ）。
    pub fn sanitize(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = CURRENT_SCHEMA_VERSION;
        }
        if self.profiles.is_empty() {
            self.profiles = Self::default().profiles;
        }
        if self.selected_preset_index >= self.profiles.len() {
            self.selected_preset_index = 0;
        }
        Self::clamp_profile(&mut self.last_form);
        for profile in &mut self.profiles {
            Self::clamp_profile(profile);
        }
        for item in &mut self.queue {
            Self::clamp_profile(&mut item.profile);
        }
    }

    fn clamp_profile(profile: &mut BackupProfile) {
        profile.multithread_count = profile.multithread_count.clamp(1, 128);
        profile.retry_count = profile.retry_count.min(1_000_000);
        profile.wait_seconds = profile.wait_seconds.min(3600);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_serialization_never_persists_shutdown_flag() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string_pretty(&cfg).expect("serialize config");
        assert!(!json.to_lowercase().contains("shutdown"));

        let parsed: AppConfig = serde_json::from_str(&json).expect("deserialize config");
        assert_eq!(parsed.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(parsed.profiles.len(), 3);
        assert!(parsed.continue_queue_on_error);
    }

    #[test]
    fn config_deserializes_partial_json_with_defaults() {
        let partial = r#"{"profiles":[{"name":"Custom","source":"C:\\Src"}]}"#;
        let mut parsed: AppConfig = serde_json::from_str(partial).expect("parse partial");
        parsed.sanitize();
        assert_eq!(parsed.profiles[0].name, "Custom");
        assert_eq!(parsed.profiles[0].multithread_count, 8);
        assert!(parsed.profiles[0].exclude_junctions);
        assert!(parsed.continue_queue_on_error);
    }
}
