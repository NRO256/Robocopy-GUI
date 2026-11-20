use super::queue::JobOutcome;

/// 単体 robocopy 終了コードの分類結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExitCodeSummary {
    pub code: u32,
    pub is_success: bool,
    pub is_warning: bool,
    pub summary_ja: String,
}

/// 終了コードをビットマスク規約に従って解析する。
pub fn classify_exit_code(code: u32) -> ExitCodeSummary {
    if code == 0 {
        return ExitCodeSummary {
            code,
            is_success: true,
            is_warning: false,
            summary_ja: "変更なし（コピー元・先が完全一致）".to_string(),
        };
    }

    let mut parts = Vec::with_capacity(6);
    if code & 1 != 0 {
        parts.push("1: ファイルのコピー成功");
    }
    if code & 2 != 0 {
        parts.push("2: コピー先にのみ存在するファイル/フォルダ検出");
    }
    if code & 4 != 0 {
        parts.push("4: 不一致ファイルを検出");
    }
    if code & 8 != 0 {
        parts.push("8: コピー失敗（リトライ上限超過）");
    }
    if code & 16 != 0 {
        parts.push("16: 致命的エラー（実行不可・アクセス拒否等）");
    }
    if code >= 32 {
        parts.push("32+: 未定義またはシステム異常終了コード");
    }

    let is_failure = code >= 8;
    let is_warning = !is_failure && (code & 4 != 0);
    let summary_ja = if is_failure {
        format!("異常終了 (コード:{code}) — {}", parts.join(" / "))
    } else if is_warning {
        format!("注意付き完了 (コード:{code}) — {}", parts.join(" / "))
    } else {
        format!("正常完了 (コード:{code}) — {}", parts.join(" / "))
    };

    ExitCodeSummary {
        code,
        is_success: !is_failure,
        is_warning,
        summary_ja,
    }
}

/// 終了コードを `JobOutcome` へ変換する。
pub fn outcome_from_exit_code(code: u32) -> JobOutcome {
    let summary = classify_exit_code(code);
    if !summary.is_success {
        JobOutcome::Failed(code)
    } else if summary.is_warning {
        JobOutcome::CompletedWithWarnings(code)
    } else {
        JobOutcome::Succeeded(code)
    }
}

/// 実行計画全体の集計結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSummary {
    pub failed_jobs: usize,
    pub max_exit_code: u32,
    pub allow_shutdown: bool,
    pub summary_line: String,
}

/// 全実行ジョブ結果を集計し、シャットダウン許可可否（全ジョブ最大終了コード < 8 かつ非中断・非ドライラン）を判定する。
pub fn summarize_plan(
    total_jobs: usize,
    outcomes: &[JobOutcome],
    is_dry_run: bool,
    shutdown_requested: bool,
) -> PlanSummary {
    let mut succeeded = 0usize;
    let mut warnings = 0usize;
    let mut failed = 0usize;
    let mut cancelled = false;
    let mut max_code = 0u32;

    for outcome in outcomes {
        match outcome {
            JobOutcome::Succeeded(c) => {
                succeeded += 1;
                max_code = max_code.max(*c);
            }
            JobOutcome::CompletedWithWarnings(c) => {
                warnings += 1;
                max_code = max_code.max(*c);
            }
            JobOutcome::Failed(c) => {
                failed += 1;
                max_code = max_code.max(*c);
            }
            JobOutcome::SpawnError(_) => {
                failed += 1;
                max_code = max_code.max(16);
            }
            JobOutcome::CancelledByUser => {
                cancelled = true;
            }
        }
    }

    let allow_shutdown = shutdown_requested
        && !is_dry_run
        && !cancelled
        && failed == 0
        && max_code < 8
        && !outcomes.is_empty();

    let mode_tag = if is_dry_run {
        "テスト実行(/L)"
    } else {
        "バックアップ"
    };
    let summary_line = if cancelled {
        format!(
            "{mode_tag}中断: 実行 {}/{} 件（成功 {succeeded}, 注意 {warnings}, 失敗 {failed}）",
            outcomes.len(),
            total_jobs
        )
    } else if failed > 0 {
        format!(
            "{mode_tag}異常あり: 実行 {}/{} 件（成功 {succeeded}, 注意 {warnings}, 失敗 {failed}, 最大コード {max_code}）",
            outcomes.len(),
            total_jobs
        )
    } else {
        format!(
            "{mode_tag}完了: {}/{} 件正常終了（最大コード {max_code}）",
            outcomes.len(),
            total_jobs
        )
    };

    PlanSummary {
        failed_jobs: failed,
        max_exit_code: max_code,
        allow_shutdown,
        summary_line,
    }
}

/// ヘルプダイアログ用の robocopy 終了コード解説本文。
pub fn exit_code_reference_text() -> &'static str {
    "robocopy.exe の終了コードはビットフラグの組み合わせです。\r\n\r\n\
     [正常系: 0〜7]\r\n\
     0 : コピー対象なし（元と先が一致）\r\n\
     1 : 1つ以上のファイルを正常にコピー\r\n\
     2 : コピー先に余分なファイルまたはフォルダが存在\r\n\
     3 : (1 + 2) コピー成功 ＋ 余分なファイル検出\r\n\
     4 : 一部のファイル/フォルダで不一致を検出\r\n\
     5〜7 : コピー成功・余分・不一致の組み合わせ\r\n\r\n\
     [異常系: 8以上]\r\n\
     8 : 一部のファイル/ディレクトリのコピーに失敗（リトライ超過）\r\n\
     16 : 致命的エラー（引数不正・パス不在・アクセス拒否など）\r\n\r\n\
     ※ 本ツールでは 0〜7 を正常終了、8以上を異常終了として判定し、\r\n\
        完了後シャットダウン有効時でも 8以上または中断時はシャットダウンを中止します。"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_0_to_7_are_success_and_8_plus_are_failure() {
        for c in 0..=7 {
            let s = classify_exit_code(c);
            assert!(s.is_success, "code {c} should be success");
        }
        for c in 8..=20 {
            let s = classify_exit_code(c);
            assert!(!s.is_success, "code {c} should be failure");
        }
    }

    #[test]
    fn plan_summary_blocks_shutdown_if_first_job_failed_even_when_last_succeeded() {
        let outcomes = vec![JobOutcome::Failed(16), JobOutcome::Succeeded(1)];
        let summary = summarize_plan(2, &outcomes, false, true);
        assert!(!summary.allow_shutdown);
        assert_eq!(summary.max_exit_code, 16);
        assert_eq!(summary.failed_jobs, 1);
    }

    #[test]
    fn plan_summary_blocks_shutdown_on_cancel_or_dry_run() {
        let ok = vec![JobOutcome::Succeeded(1)];
        assert!(!summarize_plan(1, &ok, true, true).allow_shutdown);
        let cancelled = vec![JobOutcome::CancelledByUser];
        assert!(!summarize_plan(1, &cancelled, false, true).allow_shutdown);
        assert!(summarize_plan(1, &ok, false, true).allow_shutdown);
    }
}
