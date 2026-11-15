#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::path::PathBuf;

use robocopy_gui_lib::domain::command::build_command_spec;
use robocopy_gui_lib::domain::profile::BackupProfile;
use robocopy_gui_lib::infra::storage::PortablePaths;
use robocopy_gui_lib::infra::system::is_process_elevated;
use robocopy_gui_lib::ui::{run_gui, show_fatal_error};

fn parse_restart_args() -> (bool, Option<PathBuf>) {
    let mut restarted = false;
    let mut handoff = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--restarted" {
            restarted = true;
        } else if arg == "--handoff" {
            restarted = true;
            handoff = args.next().map(PathBuf::from);
        }
    }
    (restarted, handoff)
}

fn main() {
    if std::env::args().any(|a| a == "--self-check") {
        let _paths = PortablePaths::resolve();
        let _elev = is_process_elevated();
        let p = BackupProfile {
            source: "C:\\Src".to_string(),
            destination: "D:\\Dst".to_string(),
            ..Default::default()
        };
        if build_command_spec(&p, true).is_err() {
            std::process::exit(1);
        }
        return;
    }

    std::panic::set_hook(Box::new(|info| {
        let msg = format!("予期しないエラーが発生しました:\n{info}");
        show_fatal_error("Robocopy-GUI 異常終了", &msg);
    }));

    let (is_restarted, handoff) = parse_restart_args();
    if let Err(err) = run_gui(handoff, is_restarted) {
        show_fatal_error("Robocopy-GUI 起動エラー", &err);
        std::process::exit(1);
    }
}
