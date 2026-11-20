use std::fmt::Write as _;

use super::profile::{BackupPrivilegeMode, BackupProfile, CopyMode};

/// コマンド生成時のエラー種別。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandBuildError {
    pub message: String,
    pub is_extra_args_error: bool,
}

/// 生成済み robocopy コマンド仕様。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// `robocopy.exe` に渡す引数配列（シェル非経由でそのまま渡す）。
    pub args: Vec<String>,
    /// UIプレビュー表示用の1行コマンド文字列。
    pub display_command: String,
    /// 正規化済みコピー元パス。
    pub normalized_source: String,
    /// 正規化済みコピー先パス。
    pub normalized_dest: String,
    /// `/MIR`, `/PURGE`, `/MOVE`, `/MOV` いずれかを含むか。
    pub has_destructive_flags: bool,
    /// `/MIR` または `/PURGE`（コピー先削除系）を含むか。
    pub has_mirror_or_purge: bool,
    /// `/MOVE` または `/MOV`（コピー元削除系）を含むか。
    pub has_move_flags: bool,
    /// `/B`（純粋バックアップ権限）を含むか。
    pub uses_backup_only_privilege: bool,
    /// `/ZB`（フォールバック付きバックアップ権限）を含むか。
    pub uses_zb_privilege: bool,
}

/// パスを robocopy 向けに安全正規化する。
/// - 末尾バックスラッシュによる `\"` エスケープ事故を防止する。
/// - ドライブルート `X:\` / `X:` は `X:\.` へ正規化する（`X:` カレントディレクトリ誤解釈を防止）。
/// - UNCルート `\\server\share\` は `\\server\share` へ正規化する。
pub fn normalize_robocopy_path(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut unquoted = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim()
        .replace('/', "\\");

    if unquoted.is_empty() {
        return String::new();
    }

    if unquoted
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("\\\\?\\UNC\\"))
    {
        unquoted = format!("\\\\{}", &unquoted[8..]);
    } else if let Some(stripped) = unquoted.strip_prefix("\\\\?\\") {
        unquoted = stripped.to_string();
    }

    let bytes = unquoted.as_bytes();
    // ドライブ指定: "D:", "D:\", "D:/", "D:\\\"
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let rest = &unquoted[2..];
        let rest_trimmed = rest.trim_end_matches('\\');
        if rest_trimmed.is_empty() || rest_trimmed == "." {
            let drive_char = unquoted.chars().next().unwrap_or('C');
            return format!("{drive_char}:\\.");
        }
    }

    // UNCルート判定: \\server\share\ -> \\server\share
    if let Some(unc_body) = unquoted.strip_prefix("\\\\") {
        let cleaned = unc_body.trim_end_matches('\\');
        return format!("\\\\{cleaned}");
    }

    unquoted.trim_end_matches('\\').to_string()
}

/// `/XD` 用の区切りパーサ（空白を含むフォルダ名を壊さないよう `;` と改行のみで分割）。
pub fn parse_xd_items(raw: &str) -> Vec<String> {
    raw.split([';', '\r', '\n'])
        .map(|part| {
            let t = part.trim();
            t.strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(t)
                .trim()
                .trim_end_matches(['\\', '/'])
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// `/XF` 用の区切りパーサ（`;`・改行・クォート外空白に対応）。
pub fn parse_xf_items(raw: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in raw.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            ';' | '\r' | '\n' if !in_quotes => {
                let t = current.trim();
                if !t.is_empty() {
                    items.push(t.to_string());
                }
                current.clear();
            }
            c if c.is_whitespace() && !in_quotes => {
                let t = current.trim();
                if !t.is_empty() {
                    items.push(t.to_string());
                }
                current.clear();
            }
            c => current.push(c),
        }
    }
    let t = current.trim();
    if !t.is_empty() {
        items.push(t.to_string());
    }
    items
}

fn parse_exclusion_items(raw: &str, is_xd: bool) -> Vec<String> {
    if is_xd {
        parse_xd_items(raw)
    } else {
        parse_xf_items(raw)
    }
}

/// 除外設定欄に指定トークンが含まれているか判定する（大文字小文字無視）。
pub fn has_exclusion_token(raw: &str, token: &str, is_xd: bool) -> bool {
    parse_exclusion_items(raw, is_xd)
        .iter()
        .any(|x| x.eq_ignore_ascii_case(token))
}

/// 除外設定欄のトークンをトグルし、更新後の文字列と「追加されたか(true)/削除されたか(false)」を返す。
pub fn toggle_exclusion_token(raw: &str, token: &str, is_xd: bool) -> (String, bool) {
    let mut items = parse_exclusion_items(raw, is_xd);
    let existed = items.iter().any(|x| x.eq_ignore_ascii_case(token));
    if existed {
        items.retain(|x| !x.eq_ignore_ascii_case(token));
    } else {
        items.push(token.to_string());
    }
    let formatted: Vec<String> = items
        .into_iter()
        .map(|item| {
            if !is_xd && item.contains([' ', '\t']) {
                format!("\"{item}\"")
            } else {
                item
            }
        })
        .collect();
    (formatted.join(";"), !existed)
}

fn extract_path_leaf(raw: &str) -> Option<String> {
    let norm = normalize_robocopy_path(raw);
    if norm.is_empty() {
        return None;
    }
    if let Some(drive_only) = norm.strip_suffix("\\.")
        && drive_only.len() == 2
    {
        return Some(drive_only.to_string());
    }
    norm.rsplit(['\\', '/'])
        .find(|s| !s.is_empty())
        .map(str::to_string)
        .or(Some(norm))
}

/// コピー元・先のパスから分かりやすいジョブ名を自動生成する。
pub fn suggest_job_name(source: &str, destination: &str, fallback_id: u64) -> String {
    match (extract_path_leaf(source), extract_path_leaf(destination)) {
        (Some(s), Some(d)) => format!("{s} -> {d}"),
        (Some(s), None) => s,
        (None, Some(d)) => d,
        (None, None) => format!("ジョブ {fallback_id}"),
    }
}

/// 追加カスタム引数を引用符対応でトークン分割する。
pub fn parse_custom_args(raw: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut has_token = false;

    for ch in raw.chars() {
        if ch == '\0' || ch == '\r' || ch == '\n' {
            return Err("追加引数に改行や制御文字は指定できません。".to_string());
        }
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if in_quotes {
        return Err("追加引数のダブルクォートが閉じられていません。".to_string());
    }
    if has_token {
        tokens.push(current);
    }
    Ok(tokens)
}

/// トークンを大文字キー部（例: `/LOG+:foo` -> `LOG+`）へ正規化する。
fn extract_switch_key(token: &str) -> Option<String> {
    let body = token
        .strip_prefix('/')
        .or_else(|| token.strip_prefix('-'))?;
    let key = body.split(':').next().unwrap_or(body);
    Some(key.to_ascii_uppercase())
}

/// 表示用（プレビュー欄）の1引数クォート整形。
pub fn quote_for_display(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if arg.contains([' ', '\t', '"']) {
        let escaped = arg.replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        arg.to_string()
    }
}

/// `.bat`（`cmd.exe`）用に1引数を安全整形する。改行や未エスケープ危険文字を拒否する。
pub fn format_bat_arg(arg: &str) -> Result<String, String> {
    if arg.contains(['\r', '\n', '\0']) {
        return Err("改行または制御文字を含む引数は .bat へ出力できません。".to_string());
    }
    if arg.contains('"') {
        return Err(
            "安全のためダブルクォート文字自体を含むパス・引数は .bat へ出力できません。"
                .to_string(),
        );
    }
    // cmd.exe はダブルクォート内でも %ENV% を展開するため %% へエスケープする。
    let pct_escaped = arg.replace('%', "%%");
    let needs_quotes = pct_escaped.is_empty()
        || pct_escaped.contains([
            ' ', '\t', '&', '|', '<', '>', '^', '!', '(', ')', ';', ',', '%', '=', '~',
        ]);
    if needs_quotes {
        let trailing_bs = pct_escaped.chars().rev().take_while(|&c| c == '\\').count();
        let mut quoted = String::with_capacity(pct_escaped.len() + trailing_bs + 2);
        quoted.push('"');
        quoted.push_str(&pct_escaped);
        for _ in 0..trailing_bs {
            quoted.push('\\');
        }
        quoted.push('"');
        Ok(quoted)
    } else {
        Ok(pct_escaped)
    }
}

/// プロファイルとドライラン設定から安全な `CommandSpec` を構築する。
pub fn build_command_spec(
    profile: &BackupProfile,
    dry_run: bool,
) -> Result<CommandSpec, CommandBuildError> {
    let src = normalize_robocopy_path(&profile.source);
    let dst = normalize_robocopy_path(&profile.destination);

    let custom_tokens =
        parse_custom_args(&profile.extra_args).map_err(|msg| CommandBuildError {
            message: msg,
            is_extra_args_error: true,
        })?;

    let mut custom_has_copy = false;
    let mut custom_has_mirror_or_purge = false;
    let mut custom_has_move = false;
    let mut custom_has_b = false;
    let mut custom_has_zb = false;
    let mut custom_has_mt = false;
    let mut custom_has_r = false;
    let mut custom_has_w = false;

    for tok in &custom_tokens {
        if let Some(key) = extract_switch_key(tok) {
            match key.as_str() {
                "MT" => custom_has_mt = true,
                "R" => custom_has_r = true,
                "W" => custom_has_w = true,
                "MIR" | "PURGE" => custom_has_mirror_or_purge = true,
                "MOVE" | "MOV" => custom_has_move = true,
                "B" => custom_has_b = true,
                "ZB" => custom_has_zb = true,
                "COPY" | "COPYALL" | "NOCOPY" => custom_has_copy = true,
                _ => {}
            }
        }
    }

    for tok in &custom_tokens {
        if let Some(key) = extract_switch_key(tok) {
            match key.as_str() {
                "REG" => {
                    return Err(CommandBuildError {
                        message: "ポータブル要件により /REG オプションは禁止されています。"
                            .to_string(),
                        is_extra_args_error: true,
                    });
                }
                "MON" | "MOT" | "RH" => {
                    return Err(CommandBuildError {
                        message: format!(
                            "常駐・待機系オプション /{key} はGUI実行ハング防止のため禁止されています。"
                        ),
                        is_extra_args_error: true,
                    });
                }
                "QUIT" => {
                    return Err(CommandBuildError {
                        message: "/QUIT オプションは実行を抑制するため指定できません。".to_string(),
                        is_extra_args_error: true,
                    });
                }
                "LOG" | "LOG+" | "UNILOG" | "UNILOG+" | "UNICODE" => {
                    return Err(CommandBuildError {
                        message: format!(
                            "出力制御オプション /{key} は内部ログ取得と競合するため指定できません。"
                        ),
                        is_extra_args_error: true,
                    });
                }
                "JOB" | "SAVE" => {
                    return Err(CommandBuildError {
                        message: format!("/{key} は本ツールの引数検証対象外のため指定できません。"),
                        is_extra_args_error: true,
                    });
                }
                "IPG" | "EFSRAW" | "LFSM" if profile.multithread_enabled || custom_has_mt => {
                    return Err(CommandBuildError {
                        message: format!(
                            "/{key} はマルチスレッド (/MT) と併用できません。/MT を無効にするか /{key} を削除してください。"
                        ),
                        is_extra_args_error: true,
                    });
                }
                _ => {}
            }
        }
    }

    let mut args = Vec::with_capacity(24);
    args.push(if src.is_empty() {
        "<コピー元未指定>".to_string()
    } else {
        src.clone()
    });
    args.push(if dst.is_empty() {
        "<コピー先未指定>".to_string()
    } else {
        dst.clone()
    });

    // コピーモード
    args.push(profile.copy_mode.flag().to_string());

    // 補助オプション
    if profile.exclude_junctions {
        args.push("/XJ".to_string());
    }
    if profile.fat_time_tolerance {
        args.push("/FFT".to_string());
    }
    match profile.backup_mode {
        BackupPrivilegeMode::None => {
            if profile.restartable {
                args.push("/Z".to_string());
            }
        }
        BackupPrivilegeMode::RestartableFallback => {
            args.push("/ZB".to_string());
        }
        BackupPrivilegeMode::BackupOnly => {
            if profile.restartable {
                args.push("/ZB".to_string());
            } else {
                args.push("/B".to_string());
            }
        }
    }

    // 属性コピー設定（追加引数側で /COPY 系指定時は二重指定しない）
    if !custom_has_copy {
        if profile.copy_security {
            args.push("/COPY:DATS".to_string());
        } else {
            args.push("/COPY:DAT".to_string());
        }
    }

    // 除外フォルダ /XD
    let xd_items = parse_xd_items(&profile.exclude_dirs);
    if !xd_items.is_empty() {
        args.push("/XD".to_string());
        args.extend(xd_items);
    }

    // 除外ファイル /XF
    let xf_items = parse_xf_items(&profile.exclude_files);
    if !xf_items.is_empty() {
        args.push("/XF".to_string());
        args.extend(xf_items);
    }

    // 除外属性 /XA
    let mut xa = String::new();
    if profile.exclude_hidden {
        xa.push('H');
    }
    if profile.exclude_system {
        xa.push('S');
    }
    if !xa.is_empty() {
        args.push(format!("/XA:{xa}"));
    }

    // パフォーマンス・リトライ
    if profile.multithread_enabled && !custom_has_mt {
        let mt = profile.multithread_count.clamp(1, 128);
        args.push(format!("/MT:{mt}"));
    }
    if !custom_has_r {
        args.push(format!("/R:{}", profile.retry_count));
    }
    if !custom_has_w {
        args.push(format!("/W:{}", profile.wait_seconds));
    }

    if profile.no_progress {
        args.push("/NP".to_string());
    }
    if profile.verbose {
        args.push("/V".to_string());
    }
    if dry_run {
        args.push("/L".to_string());
    }

    args.extend(custom_tokens);

    let mut display_command = String::from("robocopy.exe");
    for a in &args {
        display_command.push(' ');
        display_command.push_str(&quote_for_display(a));
    }

    let has_mirror_or_purge =
        matches!(profile.copy_mode, CopyMode::Mirror) || custom_has_mirror_or_purge;
    let has_move_flags = matches!(profile.copy_mode, CopyMode::Move) || custom_has_move;
    let has_destructive_flags = has_mirror_or_purge || has_move_flags;
    let uses_backup_only_privilege =
        (matches!(profile.backup_mode, BackupPrivilegeMode::BackupOnly) && !profile.restartable)
            || custom_has_b;
    let uses_zb_privilege = matches!(
        profile.backup_mode,
        BackupPrivilegeMode::RestartableFallback
    ) || (matches!(profile.backup_mode, BackupPrivilegeMode::BackupOnly)
        && profile.restartable)
        || custom_has_zb;

    Ok(CommandSpec {
        args,
        display_command,
        normalized_source: src,
        normalized_dest: dst,
        has_destructive_flags,
        has_mirror_or_purge,
        has_move_flags,
        uses_backup_only_privilege,
        uses_zb_privilege,
    })
}

/// 単一または複数ジョブから UTF-8 (`chcp 65001`) + CRLF の `.bat` スクリプトを生成する。
pub fn build_bat_script(
    jobs: &[(&str, &BackupProfile)],
    dry_run: bool,
    pause_at_end: bool,
    continue_on_error: bool,
) -> Result<String, CommandBuildError> {
    if jobs.is_empty() {
        return Err(CommandBuildError {
            message: "バッチ出力対象のジョブがありません。".to_string(),
            is_extra_args_error: false,
        });
    }

    let mut out = String::new();
    out.push_str("@echo off\r\n");
    out.push_str("chcp 65001 > nul\r\n");
    out.push_str("setlocal\r\n");
    out.push_str("set \"ROBOCOPY_EXE=%SystemRoot%\\System32\\robocopy.exe\"\r\n");
    out.push_str("set \"MAX_EXIT=0\"\r\n\r\n");

    for (idx, (job_name, profile)) in jobs.iter().enumerate() {
        if profile.source.trim().is_empty() || profile.destination.trim().is_empty() {
            return Err(CommandBuildError {
                message: format!(
                    "ジョブ「{}」のコピー元またはコピー先が未指定です。",
                    job_name
                ),
                is_extra_args_error: false,
            });
        }
        let spec = build_command_spec(profile, dry_run)?;
        let safe_comment_name = job_name.replace(['\r', '\n', '\0'], " ");
        let _ = writeln!(out, "rem --- Job {}: {} ---\r", idx + 1, safe_comment_name);
        out.push_str("\"%ROBOCOPY_EXE%\"");
        for arg in &spec.args {
            let formatted = format_bat_arg(arg).map_err(|msg| CommandBuildError {
                message: msg,
                is_extra_args_error: false,
            })?;
            out.push(' ');
            out.push_str(&formatted);
        }
        out.push_str("\r\n");
        out.push_str("set \"RC=%ERRORLEVEL%\"\r\n");
        out.push_str("if %RC% GTR %MAX_EXIT% set \"MAX_EXIT=%RC%\"\r\n");
        if !continue_on_error && jobs.len() > 1 {
            out.push_str("if %RC% GEQ 8 (\r\n");
            out.push_str("  echo [ERROR] Robocopy exited with code %RC%. Stopping queue.\r\n");
            out.push_str("  goto :DONE\r\n");
            out.push_str(")\r\n");
        }
        out.push_str("\r\n");
    }

    out.push_str(":DONE\r\n");
    out.push_str("echo Finished. Max Robocopy ExitCode=%MAX_EXIT%\r\n");
    if pause_at_end {
        out.push_str("pause\r\n");
    }
    out.push_str("if %MAX_EXIT% GEQ 8 exit /b %MAX_EXIT%\r\n");
    out.push_str("exit /b 0\r\n");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_handles_drive_roots_unc_and_trailing_slashes() {
        assert_eq!(normalize_robocopy_path("D:\\"), "D:\\.");
        assert_eq!(normalize_robocopy_path("d:"), "d:\\.");
        assert_eq!(normalize_robocopy_path("C:\\Src\\Sub\\"), "C:\\Src\\Sub");
        assert_eq!(
            normalize_robocopy_path(" \"C:\\My Docs\\\" "),
            "C:\\My Docs"
        );
        assert_eq!(
            normalize_robocopy_path("\\\\NAS\\Share\\"),
            "\\\\NAS\\Share"
        );
        assert_eq!(
            normalize_robocopy_path("\\\\NAS\\Share\\Bak\\"),
            "\\\\NAS\\Share\\Bak"
        );
        assert_eq!(normalize_robocopy_path("\\\\?\\D:\\"), "D:\\.");
        assert_eq!(
            normalize_robocopy_path("\\\\?\\UNC\\NAS\\Share\\"),
            "\\\\NAS\\Share"
        );
        assert_eq!(
            normalize_robocopy_path("\\\\?\\unc\\NAS\\Share\\Sub/"),
            "\\\\NAS\\Share\\Sub"
        );
        assert_eq!(normalize_robocopy_path("C:/Src/Sub/"), "C:\\Src\\Sub");
    }

    #[test]
    fn parse_xd_preserves_spaces_and_splits_semicolon_or_newline() {
        let items = parse_xd_items(".git; My Folder ;\n\"node_modules\"");
        assert_eq!(items, vec![".git", "My Folder", "node_modules"]);
    }

    #[test]
    fn parse_xf_supports_spaces_semicolons_and_quotes() {
        let items = parse_xf_items("*.tmp *.bak;\"my log.txt\"");
        assert_eq!(items, vec!["*.tmp", "*.bak", "my log.txt"]);
    }

    #[test]
    fn custom_args_rejects_reg_mon_log_and_mt_conflicts() {
        let mut p = BackupProfile {
            source: "C:\\Src".into(),
            destination: "D:\\Dst".into(),
            extra_args: "/REG".into(),
            ..Default::default()
        };
        assert!(build_command_spec(&p, false).is_err());

        p.extra_args = "/MON:5".into();
        assert!(build_command_spec(&p, false).is_err());

        p.extra_args = "/LOG:C:\\out.txt".into();
        assert!(build_command_spec(&p, false).is_err());

        p.extra_args = "/IPG:10".into();
        p.multithread_enabled = true;
        assert!(build_command_spec(&p, false).is_err());

        p.multithread_enabled = false;
        assert!(build_command_spec(&p, false).is_ok());

        p.extra_args = "/MT:16 /IPG:10".into();
        assert!(build_command_spec(&p, false).is_err());

        p.extra_args = "/IPG:10 /MT:16".into();
        assert!(build_command_spec(&p, false).is_err());
    }

    #[test]
    fn custom_args_overrides_mt_r_w_without_duplication_and_sets_move_flag() {
        let p = BackupProfile {
            source: "C:\\Src".into(),
            destination: "D:\\Dst".into(),
            extra_args: "/MT:16 /R:0 /W:1 /MOV".into(),
            ..Default::default()
        };
        let spec = build_command_spec(&p, false).expect("valid command");
        assert!(spec.has_destructive_flags);
        assert!(spec.has_move_flags);
        assert!(!spec.has_mirror_or_purge);
        assert_eq!(spec.args.iter().filter(|a| a.starts_with("/MT")).count(), 1);
        assert_eq!(spec.args.iter().filter(|a| a.starts_with("/R:")).count(), 1);
        assert_eq!(spec.args.iter().filter(|a| a.starts_with("/W:")).count(), 1);
    }

    #[test]
    fn custom_args_detects_destructive_purge_and_copyall_override() {
        let p = BackupProfile {
            source: "C:\\Src".into(),
            destination: "D:\\Dst".into(),
            extra_args: "/PURGE /COPYALL".into(),
            ..Default::default()
        };
        let spec = build_command_spec(&p, false).expect("valid command");
        assert!(spec.has_destructive_flags);
        assert!(spec.has_mirror_or_purge);
        assert!(!spec.has_move_flags);
        assert!(!spec.args.iter().any(|a| a == "/COPY:DAT"));
    }

    #[test]
    fn bat_export_escapes_percent_quotes_metachars_and_rejects_newlines() {
        assert_eq!(format_bat_arg("C:\\My & Dir").unwrap(), "\"C:\\My & Dir\"");
        assert_eq!(
            format_bat_arg("C:\\My Dir\\").unwrap(),
            "\"C:\\My Dir\\\\\""
        );
        assert_eq!(
            format_bat_arg("C:\\%TEMP%\\a").unwrap(),
            "\"C:\\%%TEMP%%\\a\""
        );
        assert_eq!(format_bat_arg("C:\\a=b\\~1").unwrap(), "\"C:\\a=b\\~1\"");
        assert!(format_bat_arg("bad\nline").is_err());

        let p = BackupProfile {
            name: "テスト".into(),
            source: "C:\\Src & Test".into(),
            destination: "D:\\".into(),
            ..Default::default()
        };
        let bat = build_bat_script(&[("ジョブ1", &p)], false, false, true).expect("bat");
        assert!(bat.starts_with("@echo off\r\nchcp 65001 > nul\r\n"));
        assert!(bat.contains("\"C:\\Src & Test\" D:\\."));
    }

    #[test]
    fn toggle_exclusion_token_adds_and_removes_case_insensitively() {
        assert!(!has_exclusion_token(".git;target", "node_modules", true));
        assert!(has_exclusion_token(".GIT;target", ".git", true));

        let (added, is_added) = toggle_exclusion_token(".git", "node_modules", true);
        assert!(is_added);
        assert_eq!(added, ".git;node_modules");

        let (removed, is_added2) = toggle_exclusion_token(".GIT;node_modules", ".git", true);
        assert!(!is_added2);
        assert_eq!(removed, "node_modules");

        let (xf_added, ok) = toggle_exclusion_token("*.tmp;\"my log.txt\"", "*.bak", false);
        assert!(ok);
        assert_eq!(xf_added, "*.tmp;\"my log.txt\";*.bak");
    }

    #[test]
    fn suggest_job_name_formats_leaves_drive_roots_and_fallbacks() {
        assert_eq!(
            suggest_job_name(r"C:\Users\Example\Documents", r"D:\", 3),
            "Documents -> D:"
        );
        assert_eq!(suggest_job_name(r"\\NAS\Share\Photos", r"", 2), "Photos");
        assert_eq!(suggest_job_name("", "", 5), "ジョブ 5");
    }
}
