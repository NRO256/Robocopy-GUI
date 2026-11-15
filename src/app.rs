use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

use crate::domain::command::build_command_spec;
use crate::domain::exit_code::{PlanSummary, classify_exit_code, summarize_plan};
use crate::domain::profile::{AppConfig, BackupProfile};
use crate::domain::queue::{ExecutionPlan, JobOutcome, QueueItem, QueueStatus};
use crate::domain::validator::{ValidationResult, validate_execution_plan};
use crate::infra::runner::{ActiveCancelHandle, RunnerEvent, spawn_robocopy_job};
use crate::infra::storage::{PortablePaths, save_config, write_execution_log};
use crate::infra::system::Win32PathInspector;
use crate::infra::win32_ffi::{ExecutionStateGuard, HWND, UINT, format_local_timestamps};

/// アプリ全体の実行状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPhase {
    Idle,
    Running,
    Cancelling,
}

/// 1回の実行完了時にUIへ返す結果サマリー。
#[derive(Debug, Clone)]
pub struct FinishedExecutionReport {
    pub summary: PlanSummary,
    pub saved_log_path: Option<String>,
    pub log_save_error: Option<String>,
}

/// アプリケーション状態（GUIコントロールから独立した純粋調停層）。
pub struct AppState {
    pub config: AppConfig,
    pub paths: PortablePaths,
    pub is_elevated: bool,
    pub phase: RunPhase,
    /// 非永続シャットダウンオプション（起動時・プリセット切替時は必ず false）。
    pub shutdown_after_completion: bool,
    pub active_plan: Option<ExecutionPlan>,
    pub current_job_idx: usize,
    pub outcomes: Vec<JobOutcome>,
    pub full_log_lines: Vec<String>,
    pub run_started_at: Option<Instant>,
    pub active_cancel: Option<ActiveCancelHandle>,
    pub runner_rx: Option<Receiver<RunnerEvent>>,
    pub exec_guard: Option<ExecutionStateGuard>,
    pub is_syncing_ui: bool,
    pub status_text: String,
    next_queue_id: u64,
}

impl AppState {
    pub fn new(config: AppConfig, paths: PortablePaths, is_elevated: bool) -> Self {
        let next_queue_id = config.queue.iter().map(|q| q.id).max().unwrap_or(0) + 1;
        Self {
            config,
            paths,
            is_elevated,
            phase: RunPhase::Idle,
            shutdown_after_completion: false,
            active_plan: None,
            current_job_idx: 0,
            outcomes: Vec::new(),
            full_log_lines: Vec::new(),
            run_started_at: None,
            active_cancel: None,
            runner_rx: None,
            exec_guard: None,
            is_syncing_ui: false,
            status_text: "待機中".to_string(),
            next_queue_id,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.phase != RunPhase::Idle
    }

    /// 現在のフォーム編集状態からプレビュー文字列を生成する。
    pub fn current_preview_command(&self) -> String {
        match build_command_spec(&self.config.last_form, false) {
            Ok(spec) => spec.display_command,
            Err(err) => format!("[設定エラー] {}", err.message),
        }
    }

    /// プリセットを選択しフォームへ反映する（シャットダウンは必ずOFFへ戻す）。
    pub fn select_preset(&mut self, index: usize) {
        if index < self.config.profiles.len() {
            self.config.selected_preset_index = index;
            self.config.last_form = self.config.profiles[index].clone();
            self.shutdown_after_completion = false;
        }
    }

    /// 現在のフォームを新しいプリセットとして保存する。
    pub fn save_as_new_preset(&mut self, name: String) -> Result<(), String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("プリセット名を入力してください。".to_string());
        }
        let mut p = self.config.last_form.clone();
        p.name = trimmed.to_string();
        self.config.last_form.name = trimmed.to_string();
        self.config.profiles.push(p);
        self.config.selected_preset_index = self.config.profiles.len() - 1;
        save_config(&self.paths.config_path, &self.config)
    }

    /// 選択中プリセットを現在のフォーム内容で上書き保存する。
    pub fn overwrite_current_preset(&mut self) -> Result<(), String> {
        let idx = self.config.selected_preset_index;
        if idx >= self.config.profiles.len() {
            return Err("上書き対象のプリセットがありません。".to_string());
        }
        let existing_name = self.config.profiles[idx].name.clone();
        let mut updated = self.config.last_form.clone();
        updated.name = existing_name;
        self.config.profiles[idx] = updated;
        save_config(&self.paths.config_path, &self.config)
    }

    /// 選択中プリセットを削除する（最低1件は残す）。
    pub fn delete_current_preset(&mut self) -> Result<(), String> {
        if self.config.profiles.len() <= 1 {
            return Err("最後のプリセットは削除できません。".to_string());
        }
        let idx = self.config.selected_preset_index;
        if idx >= self.config.profiles.len() {
            return Err("削除対象が見つかりません。".to_string());
        }
        self.config.profiles.remove(idx);
        let new_idx = idx.min(self.config.profiles.len() - 1);
        self.select_preset(new_idx);
        save_config(&self.paths.config_path, &self.config)
    }

    /// コピー元とコピー先を入れ替える。
    pub fn swap_source_and_dest(&mut self) {
        std::mem::swap(
            &mut self.config.last_form.source,
            &mut self.config.last_form.destination,
        );
    }

    /// 現在の編集内容をバックアップキュー末尾へ追加する。
    pub fn add_current_to_queue(&mut self) -> usize {
        let id = self.next_queue_id;
        self.next_queue_id += 1;
        let name = if self.config.last_form.name.trim().is_empty() {
            format!("ジョブ {id}")
        } else {
            self.config.last_form.name.clone()
        };
        self.config.queue.push(QueueItem {
            id,
            enabled: true,
            name,
            profile: self.config.last_form.clone(),
            status: QueueStatus::Pending,
        });
        self.config.queue.len() - 1
    }

    /// キュー項目をフォームへ読み込む。
    pub fn load_queue_item_to_form(&mut self, index: usize) -> bool {
        if let Some(item) = self.config.queue.get(index) {
            self.config.last_form = item.profile.clone();
            self.shutdown_after_completion = false;
            true
        } else {
            false
        }
    }

    /// キュー項目を上下移動し、移動後のインデックスを返す。
    pub fn move_queue_item(&mut self, index: usize, delta: isize) -> Option<usize> {
        let target = index as isize + delta;
        if target < 0 || target >= self.config.queue.len() as isize {
            return None;
        }
        let target = target as usize;
        self.config.queue.swap(index, target);
        Some(target)
    }

    /// 指定インデックスのキュー項目を削除する。
    pub fn delete_queue_item(&mut self, index: usize) -> bool {
        if index < self.config.queue.len() {
            self.config.queue.remove(index);
            true
        } else {
            false
        }
    }

    /// キューを全消去する。
    pub fn clear_queue(&mut self) {
        self.config.queue.clear();
    }

    /// キュー項目の有効チェックを更新する。
    pub fn set_queue_item_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(item) = self.config.queue.get_mut(index) {
            item.enabled = enabled;
        }
    }

    /// 有効なキュー項目が1件以上あるか。
    pub fn has_enabled_queue_jobs(&self) -> bool {
        self.config.queue.iter().any(|q| q.enabled)
    }

    /// 実行計画を生成し SafetyValidator で事前検証する。
    pub fn prepare_plan(
        &self,
        is_queue: bool,
        is_dry_run: bool,
    ) -> (ExecutionPlan, ValidationResult) {
        let plan = if is_queue {
            ExecutionPlan::from_queue(
                &self.config.queue,
                is_dry_run,
                self.config.continue_queue_on_error,
                self.shutdown_after_completion,
            )
        } else {
            ExecutionPlan::single(
                self.config.last_form.clone(),
                is_dry_run,
                self.shutdown_after_completion,
            )
        };
        let inspector = Win32PathInspector;
        let validation = validate_execution_plan(&plan, &inspector, self.is_elevated);
        (plan, validation)
    }

    /// 検証済み実行計画を開始する。戻り値はログ欄初期行。
    pub fn begin_execution(
        &mut self,
        plan: ExecutionPlan,
        hwnd: HWND,
        wm_notify: UINT,
    ) -> Vec<String> {
        for item in &mut self.config.queue {
            if plan.is_queue_mode {
                item.status = if item.enabled {
                    QueueStatus::Pending
                } else {
                    QueueStatus::Skipped
                };
            }
        }
        let total = plan.jobs.len();
        self.active_plan = Some(plan);
        self.current_job_idx = 0;
        self.outcomes.clear();
        self.full_log_lines.clear();
        self.run_started_at = Some(Instant::now());
        self.phase = RunPhase::Running;
        self.exec_guard = Some(ExecutionStateGuard::acquire());

        let (_, readable) = format_local_timestamps();
        let mode_str = if self.active_plan.as_ref().is_some_and(|p| p.is_dry_run) {
            "テスト実行 (/L)"
        } else {
            "バックアップ実行"
        };
        let header = vec![format!(
            "=== Robocopy-GUI {mode_str} 開始 ({readable}, 対象 {total} 件) ==="
        )];
        self.full_log_lines.extend(header.clone());

        let mut start_lines = header;
        start_lines.extend(self.dispatch_current_job(hwnd, wm_notify));
        start_lines
    }

    fn dispatch_current_job(&mut self, hwnd: HWND, wm_notify: UINT) -> Vec<String> {
        let Some(plan) = self.active_plan.as_ref() else {
            return Vec::new();
        };
        let total = plan.jobs.len();
        let idx = self.current_job_idx;
        let job = &plan.jobs[idx];

        if let Some(q_idx) = job.queue_index
            && let Some(item) = self.config.queue.get_mut(q_idx)
        {
            item.status = QueueStatus::Running;
        }

        let mode_label = if plan.is_dry_run {
            "テスト(/L)"
        } else {
            "実行中"
        };
        self.status_text = format!("{mode_label} [{}/{}] {}", idx + 1, total, job.display_name);

        let Ok(spec) = build_command_spec(&job.profile, plan.is_dry_run) else {
            let err_outcome = JobOutcome::SpawnError("コマンド生成に失敗しました。".to_string());
            let mut lines = vec![format!(
                "[ジョブ {}/{}] コマンド生成失敗: {}",
                idx + 1,
                total,
                job.display_name
            )];
            self.full_log_lines.extend(lines.clone());
            self.outcomes.push(err_outcome);
            lines.push("終了を処理しています...".to_string());
            return lines;
        };

        let (_, readable) = format_local_timestamps();
        let job_header = vec![
            String::new(),
            format!(
                "--- ジョブ {}/{} [{}] ({readable}) ---",
                idx + 1,
                total,
                job.display_name
            ),
            format!("コマンド: {}", spec.display_command),
        ];
        self.full_log_lines.extend(job_header.clone());

        let (tx, rx) = mpsc::channel();
        let cancel = ActiveCancelHandle::new();
        self.runner_rx = Some(rx);
        self.active_cancel = Some(cancel.clone());
        spawn_robocopy_job(hwnd, wm_notify, spec.args, tx, cancel);

        job_header
    }

    /// 実行中止を要求する。
    pub fn request_cancel(&mut self) {
        if self.phase == RunPhase::Running {
            self.phase = RunPhase::Cancelling;
            self.status_text = "中止処理中...".to_string();
            if let Some(cancel) = &self.active_cancel {
                cancel.request_cancel();
            }
        }
    }

    /// 経過時間を含めた最新ステータス表示テキストを返す。
    pub fn formatted_running_status(&self) -> String {
        if let Some(started) = self.run_started_at {
            let secs = started.elapsed().as_secs();
            let mins = secs / 60;
            let rem = secs % 60;
            format!("{} | 経過 {:02}:{:02}", self.status_text, mins, rem)
        } else {
            self.status_text.clone()
        }
    }

    /// ワーカースレッドからのイベントを処理し、新規ログ行と（全完了時）完了レポートを返す。
    pub fn process_runner_events(
        &mut self,
        hwnd: HWND,
        wm_notify: UINT,
    ) -> (Vec<String>, Option<FinishedExecutionReport>, bool) {
        let mut new_lines = Vec::new();
        let mut finished_outcome: Option<JobOutcome> = None;

        if let Some(rx) = &self.runner_rx {
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    RunnerEvent::LogLines(lines) => {
                        self.full_log_lines.extend(lines.iter().cloned());
                        new_lines.extend(lines);
                    }
                    RunnerEvent::JobFinished(outcome) => {
                        finished_outcome = Some(outcome);
                    }
                }
            }
        }

        let mut queue_updated = false;
        let Some(outcome) = finished_outcome else {
            return (new_lines, None, queue_updated);
        };

        self.runner_rx = None;
        self.active_cancel = None;

        if let Some(plan) = self.active_plan.as_ref()
            && let Some(job) = plan.jobs.get(self.current_job_idx)
            && let Some(q_idx) = job.queue_index
            && let Some(item) = self.config.queue.get_mut(q_idx)
        {
            item.status = outcome.to_queue_status();
            queue_updated = true;
        }

        let outcome_line = match &outcome {
            JobOutcome::Succeeded(c)
            | JobOutcome::CompletedWithWarnings(c)
            | JobOutcome::Failed(c) => {
                let s = classify_exit_code(*c);
                format!(">> 結果: {}", s.summary_ja)
            }
            JobOutcome::CancelledByUser => {
                ">> 結果: ユーザー操作により中断されました。".to_string()
            }
            JobOutcome::SpawnError(err) => format!(">> 結果: 起動エラー ({err})"),
        };
        new_lines.push(outcome_line.clone());
        self.full_log_lines.push(outcome_line);

        let should_stop_early = outcome.is_cancelled()
            || (outcome.is_failure()
                && !self
                    .active_plan
                    .as_ref()
                    .is_some_and(|p| p.continue_on_error));
        self.outcomes.push(outcome);

        let has_more_jobs = self
            .active_plan
            .as_ref()
            .is_some_and(|p| self.current_job_idx + 1 < p.jobs.len());

        if !should_stop_early && has_more_jobs {
            self.current_job_idx += 1;
            let next_lines = self.dispatch_current_job(hwnd, wm_notify);
            new_lines.extend(next_lines);
            return (new_lines, None, true);
        }

        // 実行計画全体が完了（または中断・途中停止）
        let plan = self.active_plan.take();
        let (total_jobs, is_dry_run, shutdown_req, should_save_log) = match &plan {
            Some(p) => {
                let any_auto_log = p.jobs.iter().any(|j| j.profile.auto_save_log);
                (
                    p.jobs.len(),
                    p.is_dry_run,
                    p.shutdown_after_completion,
                    any_auto_log,
                )
            }
            None => (self.outcomes.len(), false, false, true),
        };

        let summary = summarize_plan(total_jobs, &self.outcomes, is_dry_run, shutdown_req);
        let footer = format!("=== {} ===", summary.summary_line);
        new_lines.push(footer.clone());
        self.full_log_lines.push(footer);

        // 残余ジョブが途中停止で未実行なら Skipped に更新
        if let Some(p) = &plan {
            for job in p.jobs.iter().skip(self.outcomes.len()) {
                if let Some(q_idx) = job.queue_index
                    && let Some(item) = self.config.queue.get_mut(q_idx)
                {
                    item.status = QueueStatus::Skipped;
                    queue_updated = true;
                }
            }
        }

        self.phase = RunPhase::Idle;
        self.run_started_at = None;
        self.exec_guard = None;
        self.status_text = summary.summary_line.clone();

        let (saved_log_path, log_save_error) = if should_save_log {
            let (stamp, _) = format_local_timestamps();
            match write_execution_log(&self.paths.logs_dir, &stamp, &self.full_log_lines) {
                Ok(p) => (Some(p.to_string_lossy().to_string()), None),
                Err(e) => (None, Some(e)),
            }
        } else {
            (None, None)
        };

        if let Some(ref lp) = saved_log_path {
            let line = format!("ログ保存先: {lp}");
            new_lines.push(line);
        } else if let Some(ref err) = log_save_error {
            let line = format!("[警告] ログ保存失敗: {err}");
            new_lines.push(line);
        }

        let report = FinishedExecutionReport {
            summary,
            saved_log_path,
            log_save_error,
        };
        (new_lines, Some(report), queue_updated)
    }

    /// 新規フォーム用の初期プロファイルを編集する。
    pub fn update_form_profile(&mut self, updated: BackupProfile) {
        self.config.last_form = updated;
    }
}
