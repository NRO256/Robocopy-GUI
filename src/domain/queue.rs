use serde::{Deserialize, Serialize};

use super::profile::BackupProfile;

/// キュー内ジョブの実行ステータス（永続化対象外）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueueStatus {
    #[default]
    Pending,
    Running,
    Succeeded(u32),
    CompletedWithWarnings(u32),
    Failed(u32),
    Cancelled,
    Skipped,
}

impl QueueStatus {
    /// UIリスト表示用の日本語ステータス文字列。
    pub fn display_text(self) -> String {
        match self {
            Self::Pending => "待機中".to_string(),
            Self::Running => "実行中...".to_string(),
            Self::Succeeded(code) => format!("成功 (コード:{code})"),
            Self::CompletedWithWarnings(code) => format!("注意完了 (コード:{code})"),
            Self::Failed(code) => format!("失敗 (コード:{code})"),
            Self::Cancelled => "ユーザー中断".to_string(),
            Self::Skipped => "スキップ".to_string(),
        }
    }
}

/// 1ジョブの最終実行結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobOutcome {
    /// 終了コード 0〜3（コピーなし〜正常コピー完了）。
    Succeeded(u32),
    /// 終了コード 4〜7（不一致検出を含むが致命失敗なし）。
    CompletedWithWarnings(u32),
    /// 終了コード 8 以上（コピー失敗・致命的エラー）。
    Failed(u32),
    /// ユーザー操作による強制中断（終了コードに依存せず明確に分離）。
    CancelledByUser,
    /// プロセス起動エラー等。
    SpawnError(String),
}

impl JobOutcome {
    /// 致命的失敗または起動失敗かどうか（シャットダウン抑止判定に使用）。
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed(_) | Self::SpawnError(_))
    }

    /// ユーザー中断かどうか。
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::CancelledByUser)
    }

    /// 対応するキュー表示ステータスへ変換する。
    pub fn to_queue_status(&self) -> QueueStatus {
        match self {
            Self::Succeeded(c) => QueueStatus::Succeeded(*c),
            Self::CompletedWithWarnings(c) => QueueStatus::CompletedWithWarnings(*c),
            Self::Failed(c) => QueueStatus::Failed(*c),
            Self::CancelledByUser => QueueStatus::Cancelled,
            Self::SpawnError(_) => QueueStatus::Failed(16),
        }
    }
}

fn default_true() -> bool {
    true
}

/// バックアップキュー内の1項目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QueueItem {
    pub id: u64,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub name: String,
    pub profile: BackupProfile,
    #[serde(skip)]
    pub status: QueueStatus,
}

impl Default for QueueItem {
    fn default() -> Self {
        Self {
            id: 0,
            enabled: true,
            name: "無題ジョブ".to_string(),
            profile: BackupProfile::default(),
            status: QueueStatus::Pending,
        }
    }
}

/// 単発実行・キュー連続実行を統一パイプラインで扱う実行計画。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedJob {
    /// キューモード時の元キュー配列インデックス。単発実行時は `None`。
    pub queue_index: Option<usize>,
    pub display_name: String,
    pub profile: BackupProfile,
}

/// 実行計画全体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    pub is_dry_run: bool,
    pub is_queue_mode: bool,
    pub continue_on_error: bool,
    pub shutdown_after_completion: bool,
    pub jobs: Vec<PlannedJob>,
}

impl ExecutionPlan {
    /// 単発バックアップを要素数1の実行計画として構築する。
    pub fn single(
        profile: BackupProfile,
        is_dry_run: bool,
        shutdown_after_completion: bool,
    ) -> Self {
        let name = if profile.name.trim().is_empty() {
            "単発バックアップ".to_string()
        } else {
            profile.name.clone()
        };
        Self {
            is_dry_run,
            is_queue_mode: false,
            continue_on_error: false,
            shutdown_after_completion: if is_dry_run {
                false
            } else {
                shutdown_after_completion
            },
            jobs: vec![PlannedJob {
                queue_index: None,
                display_name: name,
                profile,
            }],
        }
    }

    /// キュー内の指定1ジョブを実行計画として構築する。
    pub fn single_queue_item(
        queue_index: usize,
        item: &QueueItem,
        is_dry_run: bool,
        shutdown_after_completion: bool,
    ) -> Self {
        Self {
            is_dry_run,
            is_queue_mode: false,
            continue_on_error: false,
            shutdown_after_completion: if is_dry_run {
                false
            } else {
                shutdown_after_completion
            },
            jobs: vec![PlannedJob {
                queue_index: Some(queue_index),
                display_name: item.name.clone(),
                profile: item.profile.clone(),
            }],
        }
    }

    /// 有効化されたキュー項目から実行計画を構築する。
    pub fn from_queue(
        queue: &[QueueItem],
        is_dry_run: bool,
        continue_on_error: bool,
        shutdown_after_completion: bool,
    ) -> Self {
        let jobs = queue
            .iter()
            .enumerate()
            .filter(|(_, item)| item.enabled)
            .map(|(idx, item)| PlannedJob {
                queue_index: Some(idx),
                display_name: item.name.clone(),
                profile: item.profile.clone(),
            })
            .collect();

        Self {
            is_dry_run,
            is_queue_mode: true,
            continue_on_error,
            shutdown_after_completion: if is_dry_run {
                false
            } else {
                shutdown_after_completion
            },
            jobs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_forces_shutdown_off() {
        let plan = ExecutionPlan::single(BackupProfile::default(), true, true);
        assert!(!plan.shutdown_after_completion);
        assert_eq!(plan.jobs.len(), 1);
    }

    #[test]
    fn queue_plan_includes_only_enabled_items_with_original_indices() {
        let q = vec![
            QueueItem {
                id: 1,
                enabled: false,
                name: "Disabled".into(),
                ..Default::default()
            },
            QueueItem {
                id: 2,
                enabled: true,
                name: "EnabledA".into(),
                ..Default::default()
            },
            QueueItem {
                id: 3,
                enabled: true,
                name: "EnabledB".into(),
                ..Default::default()
            },
        ];
        let plan = ExecutionPlan::from_queue(&q, false, true, true);
        assert_eq!(plan.jobs.len(), 2);
        assert_eq!(plan.jobs[0].queue_index, Some(1));
        assert_eq!(plan.jobs[1].queue_index, Some(2));
        assert!(plan.shutdown_after_completion);
    }

    #[test]
    fn single_queue_item_targets_only_selected_index() {
        let item = QueueItem {
            id: 10,
            enabled: false,
            name: "個別実行".into(),
            ..Default::default()
        };
        let plan = ExecutionPlan::single_queue_item(2, &item, true, true);
        assert!(plan.is_dry_run);
        assert!(!plan.is_queue_mode);
        assert!(!plan.shutdown_after_completion);
        assert_eq!(plan.jobs.len(), 1);
        assert_eq!(plan.jobs[0].queue_index, Some(2));
        assert_eq!(plan.jobs[0].display_name, "個別実行");
    }
}
