use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use robocopy_gui_lib::domain::command::{build_bat_script, build_command_spec};
use robocopy_gui_lib::domain::exit_code::{
    classify_exit_code, outcome_from_exit_code, summarize_plan,
};
use robocopy_gui_lib::domain::profile::{AppConfig, BackupProfile, CopyMode};
use robocopy_gui_lib::domain::queue::ExecutionPlan;
use robocopy_gui_lib::domain::validator::validate_execution_plan;
use robocopy_gui_lib::infra::runner::Utf16LeStreamDecoder;
use robocopy_gui_lib::infra::storage::{load_config, save_config, write_execution_log};
use robocopy_gui_lib::infra::system::Win32PathInspector;
use robocopy_gui_lib::infra::win32_ffi::get_system32_dir;

fn make_sandbox(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("robocopy_gui_it_{name}_{nanos}"));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn integration_dry_run_copy_mir_and_exclusions_with_real_robocopy() {
    let root = make_sandbox("real_rc");
    let src = root.join("元 フォルダ");
    let dst = root.join("先 フォルダ");
    let skip_dir = src.join("node_modules");
    std::fs::create_dir_all(&skip_dir).unwrap();
    std::fs::create_dir_all(&dst).unwrap();

    std::fs::write(src.join("日本語テスト.txt"), b"hello").unwrap();
    std::fs::write(src.join("ignore.tmp"), b"tmp").unwrap();
    std::fs::write(skip_dir.join("pkg.json"), b"{}").unwrap();
    std::fs::write(dst.join("extra_old.txt"), b"stale").unwrap();

    let robocopy = get_system32_dir().unwrap().join("robocopy.exe");
    let unilog = root.join("unilog.log");

    // 1. /L ドライラン（/XD, /XF 付き）でコピー先が変更されないこと
    let profile = BackupProfile {
        name: "テスト1".into(),
        source: src.to_string_lossy().to_string(),
        destination: dst.to_string_lossy().to_string(),
        copy_mode: CopyMode::IncludeEmpty,
        exclude_dirs: "node_modules".into(),
        exclude_files: "*.tmp".into(),
        retry_count: 1,
        wait_seconds: 1,
        ..Default::default()
    };

    let plan = ExecutionPlan::single(profile.clone(), true, true);
    assert!(!plan.shutdown_after_completion);
    let validation = validate_execution_plan(&plan, &Win32PathInspector, false);
    assert!(validation.is_ok(), "errors: {:?}", validation.errors);

    let mut args = build_command_spec(&profile, true).unwrap().args;
    args.push(format!("/UNILOG:{}", unilog.to_string_lossy()));
    let out = Command::new(&robocopy).args(&args).output().unwrap();
    let code = out.status.code().unwrap_or(16) as u32;
    assert!(
        classify_exit_code(code).is_success,
        "dry-run exit code={code}"
    );
    assert!(
        !dst.join("日本語テスト.txt").exists(),
        "dry run must not copy"
    );

    let raw_log = std::fs::read(&unilog).unwrap();
    let mut dec = Utf16LeStreamDecoder::default();
    let mut lines = dec.push_bytes(&raw_log);
    lines.extend(dec.finish());
    let joined = lines.join("\n");
    assert!(
        joined.contains("日本語テスト.txt"),
        "log missing Japanese filename"
    );

    // 2. 本実行（/MIR）で日本語ファイルのみコピーされ extra_old.txt が削除、除外が維持されること
    let mut mir_profile = profile;
    mir_profile.copy_mode = CopyMode::Mirror;
    let mir_args = build_command_spec(&mir_profile, false).unwrap().args;
    let mir_out = Command::new(&robocopy).args(&mir_args).output().unwrap();
    let mir_code = mir_out.status.code().unwrap_or(16) as u32;
    assert!(
        classify_exit_code(mir_code).is_success,
        "mir exit code={mir_code}"
    );
    assert!(dst.join("日本語テスト.txt").exists());
    assert!(!dst.join("ignore.tmp").exists());
    assert!(!dst.join("node_modules").exists());
    assert!(!dst.join("extra_old.txt").exists());

    let outcome = outcome_from_exit_code(mir_code);
    let summary = summarize_plan(1, &[outcome], false, true);
    assert!(summary.allow_shutdown);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn integration_config_roundtrip_bat_and_log_storage() {
    let root = make_sandbox("storage");
    let cfg_path = root.join("Robocopy-GUI_config.json");
    let logs_dir = root.join("logs");

    let mut cfg = AppConfig::default();
    cfg.last_form.source = "C:\\Work\\日本語".into();
    cfg.last_form.destination = "D:\\".into();
    save_config(&cfg_path, &cfg).unwrap();

    let (loaded, warn) = load_config(&cfg_path);
    assert!(warn.is_none());
    assert_eq!(loaded.last_form.source, "C:\\Work\\日本語");

    let bat = build_bat_script(&[("ジョブ1", &loaded.last_form)], false, false, true).unwrap();
    assert!(bat.contains("chcp 65001 > nul"));
    assert!(bat.contains("D:\\."));

    let saved_log =
        write_execution_log(&logs_dir, "20261116_070000", &["テストログ行".to_string()]).unwrap();
    assert!(saved_log.exists());

    let _ = std::fs::remove_dir_all(&root);
}
