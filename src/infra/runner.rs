use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

use crate::domain::exit_code::outcome_from_exit_code;
use crate::domain::queue::JobOutcome;

use super::win32_ffi::*;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// ワーカースレッドからUIスレッドへ送るイベント。
#[derive(Debug, Clone)]
pub enum RunnerEvent {
    LogLines(Vec<String>),
    JobFinished(JobOutcome),
}

/// UTF-16LE ストリームをチャンク境界（奇数バイト・サロゲートペア・BOM）安全に復号し行分割する。
#[derive(Default)]
pub struct Utf16LeStreamDecoder {
    bom_checked: bool,
    pending_byte: Option<u8>,
    pending_high_surrogate: Option<u16>,
    line_buf: String,
}

impl Utf16LeStreamDecoder {
    pub fn push_bytes(&mut self, bytes: &[u8]) -> Vec<String> {
        let combined;
        let mut slice = if let Some(b) = self.pending_byte.take() {
            let mut v = Vec::with_capacity(bytes.len() + 1);
            v.push(b);
            v.extend_from_slice(bytes);
            combined = v;
            combined.as_slice()
        } else {
            bytes
        };

        if !self.bom_checked && slice.len() >= 2 {
            self.bom_checked = true;
            if slice[0] == 0xFF && slice[1] == 0xFE {
                slice = &slice[2..];
            }
        }

        if slice.len() % 2 == 1 {
            self.pending_byte = Some(slice[slice.len() - 1]);
            slice = &slice[..slice.len() - 1];
        }

        let mut units = Vec::with_capacity(slice.len() / 2 + 1);
        if let Some(hi) = self.pending_high_surrogate.take() {
            units.push(hi);
        }
        for pair in slice.as_chunks::<2>().0 {
            units.push(u16::from_le_bytes([pair[0], pair[1]]));
        }

        if let Some(&last) = units.last()
            && (0xD800..=0xDBFF).contains(&last)
        {
            self.pending_high_surrogate = units.pop();
        }

        let decoded = String::from_utf16_lossy(&units);
        self.push_decoded_text(&decoded)
    }

    fn push_decoded_text(&mut self, text: &str) -> Vec<String> {
        let mut lines = Vec::new();
        for ch in text.chars() {
            match ch {
                '\r' => {}
                '\n' => {
                    lines.push(std::mem::take(&mut self.line_buf));
                }
                c => self.line_buf.push(c),
            }
        }
        lines
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(hi) = self.pending_high_surrogate.take() {
            self.line_buf.push_str(&String::from_utf16_lossy(&[hi]));
        }
        if self.pending_byte.take().is_some() {
            self.line_buf.push('\u{FFFD}');
        }
        if !self.line_buf.is_empty() {
            out.push(std::mem::take(&mut self.line_buf));
        }
        out
    }
}

/// Windows `CreateProcessW` 用の安全な1コマンドライン文字列を構築する。
pub fn build_windows_command_line(exe: &Path, args: &[String]) -> String {
    let mut cmd = quote_windows_arg(&exe.to_string_lossy());
    for arg in args {
        cmd.push(' ');
        cmd.push_str(&quote_windows_arg(arg));
    }
    cmd
}

fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '"']) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 4);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            for _ in 0..(backslashes * 2 + 1) {
                out.push('\\');
            }
            out.push('"');
            backslashes = 0;
        } else {
            for _ in 0..backslashes {
                out.push('\\');
            }
            backslashes = 0;
            out.push(ch);
        }
    }
    for _ in 0..(backslashes * 2) {
        out.push('\\');
    }
    out.push('"');
    out
}

/// 実行中ジョブを中断するための制御ハンドル。
#[derive(Clone)]
pub struct ActiveCancelHandle {
    pub cancelled: Arc<AtomicBool>,
    pub job_handle: Arc<std::sync::atomic::AtomicIsize>,
}

impl ActiveCancelHandle {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            job_handle: Arc::new(std::sync::atomic::AtomicIsize::new(0)),
        }
    }

    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        let h_job = self.job_handle.load(Ordering::SeqCst);
        if h_job != 0 {
            unsafe {
                // Robocopy の成功コード(0..7)と紛れない独自終了コードで強制終了する。
                TerminateJobObject(h_job, 0xC000_013A);
            }
        }
    }
}

impl Default for ActiveCancelHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// System32 の `robocopy.exe` を非同期起動し、`/UNILOG:` (真の UTF-16LE) を逐次読み取りしつつ Job Object でライフサイクル束縛する。
pub fn spawn_robocopy_job(
    hwnd_notify: HWND,
    wm_notify: UINT,
    args: Vec<String>,
    tx: Sender<RunnerEvent>,
    cancel_handle: ActiveCancelHandle,
) {
    thread::spawn(move || {
        let outcome = run_robocopy_blocking(hwnd_notify, wm_notify, args, &tx, &cancel_handle);
        let _ = tx.send(RunnerEvent::JobFinished(outcome));
        unsafe {
            PostMessageW(hwnd_notify, wm_notify, 0, 0);
        }
    });
}

fn run_robocopy_blocking(
    hwnd_notify: HWND,
    wm_notify: UINT,
    mut args: Vec<String>,
    tx: &Sender<RunnerEvent>,
    cancel_handle: &ActiveCancelHandle,
) -> JobOutcome {
    let sys32 = match get_system32_dir() {
        Ok(d) => d,
        Err(e) => return JobOutcome::SpawnError(e),
    };
    let robocopy_exe = sys32.join("robocopy.exe");
    if !robocopy_exe.exists() {
        return JobOutcome::SpawnError(format!(
            "robocopy.exe が見つかりません: {}",
            robocopy_exe.display()
        ));
    }

    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let temp_log_path = std::env::temp_dir().join(format!(
        "RobocopyGUI_unilog_{}_{}.log",
        std::process::id(),
        seq
    ));
    let _temp_cleanup = TempFileGuard(temp_log_path.clone());

    // Windows 11 の /UNICODE パイプは CP932 出力バグがあるため、真の UTF-16LE を出力する /UNILOG: を内部使用する。
    args.push(format!("/UNILOG:{}", temp_log_path.to_string_lossy()));

    let mut sa = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: TRUE,
    };
    let mut err_read: HANDLE = 0;
    let mut err_write: HANDLE = 0;
    if unsafe { CreatePipe(&raw mut err_read, &raw mut err_write, &raw mut sa, 0) } == 0 {
        return JobOutcome::SpawnError("標準エラーパイプの作成に失敗しました。".to_string());
    }
    let _err_read_guard = HandleGuard(err_read);
    let mut err_write_opt = Some(HandleGuard(err_write));
    unsafe {
        SetHandleInformation(err_read, HANDLE_FLAG_INHERIT, 0);
    }

    let mut out_read: HANDLE = 0;
    let mut out_write: HANDLE = 0;
    if unsafe { CreatePipe(&raw mut out_read, &raw mut out_write, &raw mut sa, 0) } == 0 {
        return JobOutcome::SpawnError("標準出力パイプの作成に失敗しました。".to_string());
    }
    let _out_read_guard = HandleGuard(out_read);
    let mut out_write_opt = Some(HandleGuard(out_write));
    unsafe {
        SetHandleInformation(out_read, HANDLE_FLAG_INHERIT, 0);
    }

    let h_job = unsafe { CreateJobObjectW(null_mut(), std::ptr::null()) };
    let _job_guard = if h_job != 0 {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                h_job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as DWORD,
            );
        }
        cancel_handle.job_handle.store(h_job, Ordering::SeqCst);
        Some(HandleGuard(h_job))
    } else {
        None
    };

    let si = STARTUPINFOW {
        dwFlags: STARTF_USESTDHANDLES,
        hStdInput: 0,
        hStdOutput: out_write,
        hStdError: err_write,
        ..Default::default()
    };
    let mut pi = PROCESS_INFORMATION::default();

    let cmd_line_str = build_windows_command_line(&robocopy_exe, &args);
    let mut cmd_line_w = to_wide(&cmd_line_str);
    let exe_w = to_wide(&robocopy_exe.to_string_lossy());

    let created = unsafe {
        CreateProcessW(
            exe_w.as_ptr(),
            cmd_line_w.as_mut_ptr(),
            null_mut(),
            null_mut(),
            TRUE,
            CREATE_NO_WINDOW | CREATE_SUSPENDED,
            null_mut(),
            std::ptr::null(),
            &raw const si,
            &raw mut pi,
        )
    };
    // 親側のパイプ書き込み端を即座に閉じ、ReadFileのEOF永久ブロックを防ぐ。
    drop(err_write_opt.take());
    drop(out_write_opt.take());

    if created == 0 {
        cancel_handle.job_handle.store(0, Ordering::SeqCst);
        let code = unsafe { GetLastError() };
        return JobOutcome::SpawnError(format!(
            "robocopy.exe の起動に失敗しました (Win32エラー={code})。"
        ));
    }

    let _proc_guard = HandleGuard(pi.hProcess);
    let _thread_guard = HandleGuard(pi.hThread);

    if h_job != 0 {
        unsafe {
            AssignProcessToJobObject(h_job, pi.hProcess);
        }
    }
    unsafe {
        ResumeThread(pi.hThread);
    }

    // stdout/stderr のフォールバック吸出しスレッド（引数エラー等で /UNILOG 生成前に出た場合用）
    let stderr_buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let stderr_buf_c = Arc::clone(&stderr_buf);
    let err_thread = thread::spawn(move || drain_pipe(err_read, &stderr_buf_c));
    let stdout_buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let stdout_buf_c = Arc::clone(&stdout_buf);
    let out_thread = thread::spawn(move || drain_pipe(out_read, &stdout_buf_c));

    let mut decoder = Utf16LeStreamDecoder::default();
    let mut unilog_file: Option<File> = None;
    let mut read_buf = Vec::with_capacity(8192);
    let mut read_offset: u64 = 0;

    loop {
        if cancel_handle.cancelled.load(Ordering::SeqCst) {
            if h_job != 0 {
                unsafe {
                    TerminateJobObject(h_job, 0xC000_013A);
                }
            }
            unsafe {
                TerminateProcess(pi.hProcess, 0xC000_013A);
            }
        }

        let new_lines = poll_unilog_lines(
            &temp_log_path,
            &mut unilog_file,
            &mut read_buf,
            &mut read_offset,
            &mut decoder,
        );
        if !new_lines.is_empty() {
            let _ = tx.send(RunnerEvent::LogLines(new_lines));
            unsafe {
                PostMessageW(hwnd_notify, wm_notify, 0, 0);
            }
        }

        let wait_res = unsafe { WaitForSingleObject(pi.hProcess, 80) };
        if wait_res == WAIT_OBJECT_0 {
            break;
        }
    }

    let _ = err_thread.join();
    let _ = out_thread.join();
    cancel_handle.job_handle.store(0, Ordering::SeqCst);

    // 終了後の残余ログを取り切り、一時ファイル削除前にハンドルを明示解放する
    let mut tail_lines = poll_unilog_lines(
        &temp_log_path,
        &mut unilog_file,
        &mut read_buf,
        &mut read_offset,
        &mut decoder,
    );
    drop(unilog_file);
    tail_lines.extend(decoder.finish());

    if read_offset == 0
        && tail_lines.is_empty()
        && let Ok(bytes) = stdout_buf.lock()
        && !bytes.is_empty()
    {
        let s = decode_acp_or_utf8(&bytes);
        for l in s.lines() {
            if !l.trim().is_empty() {
                tail_lines.push(l.to_string());
            }
        }
    }
    if let Ok(bytes) = stderr_buf.lock()
        && !bytes.is_empty()
    {
        let s = decode_acp_or_utf8(&bytes);
        for l in s.lines() {
            if !l.trim().is_empty() {
                tail_lines.push(format!("[stderr] {l}"));
            }
        }
    }
    if !tail_lines.is_empty() {
        let _ = tx.send(RunnerEvent::LogLines(tail_lines));
    }

    if cancel_handle.cancelled.load(Ordering::SeqCst) {
        return JobOutcome::CancelledByUser;
    }

    let mut exit_code: DWORD = 16;
    unsafe {
        GetExitCodeProcess(pi.hProcess, &raw mut exit_code);
    }
    outcome_from_exit_code(exit_code)
}

fn drain_pipe(pipe_read: HANDLE, sink: &Arc<std::sync::Mutex<Vec<u8>>>) {
    let mut buf = [0u8; 4096];
    loop {
        let mut read_len: DWORD = 0;
        let ok = unsafe {
            ReadFile(
                pipe_read,
                buf.as_mut_ptr().cast(),
                buf.len() as DWORD,
                &raw mut read_len,
                null_mut(),
            )
        };
        if ok == 0 || read_len == 0 {
            break;
        }
        if let Ok(mut guard) = sink.lock() {
            guard.extend_from_slice(&buf[..read_len as usize]);
        }
    }
}

fn poll_unilog_lines(
    path: &Path,
    file_opt: &mut Option<File>,
    read_buf: &mut Vec<u8>,
    offset: &mut u64,
    decoder: &mut Utf16LeStreamDecoder,
) -> Vec<String> {
    if file_opt.is_none() {
        let Ok(file) = OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .open(path)
        else {
            return Vec::new();
        };
        *file_opt = Some(file);
    }

    let Some(file) = file_opt.as_mut() else {
        return Vec::new();
    };
    read_buf.clear();
    if file.read_to_end(read_buf).is_err() || read_buf.is_empty() {
        return Vec::new();
    }
    *offset += read_buf.len() as u64;
    decoder.push_bytes(read_buf)
}

fn decode_acp_or_utf8(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let needed = unsafe {
        MultiByteToWideChar(CP_ACP, 0, bytes.as_ptr(), bytes.len() as INT, null_mut(), 0)
    };
    if needed <= 0 {
        return String::from_utf8_lossy(bytes).to_string();
    }
    let mut wide = vec![0u16; needed as usize];
    let written = unsafe {
        MultiByteToWideChar(
            CP_ACP,
            0,
            bytes.as_ptr(),
            bytes.len() as INT,
            wide.as_mut_ptr(),
            needed,
        )
    };
    if written <= 0 {
        return String::from_utf8_lossy(bytes).to_string();
    }
    String::from_utf16_lossy(&wide[..written as usize])
}

struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16le_stream_decoder_handles_bom_odd_bytes_and_surrogates() {
        let sample = "行1: 日本語テスト🚀\r\n行2: 完了\n";
        let mut raw = vec![0xFFu8, 0xFEu8];
        for u in sample.encode_utf16() {
            raw.extend_from_slice(&u.to_le_bytes());
        }

        // 1バイトずつ〜奇数チャンクで分割入力しても壊れないことを確認
        let mut dec = Utf16LeStreamDecoder::default();
        let mut lines = Vec::new();
        for chunk in raw.chunks(3) {
            lines.extend(dec.push_bytes(chunk));
        }
        lines.extend(dec.finish());
        assert_eq!(lines, vec!["行1: 日本語テスト🚀", "行2: 完了"]);
    }

    #[test]
    fn windows_command_line_quoting_handles_spaces_and_trailing_slashes() {
        let cmd = build_windows_command_line(
            Path::new("C:\\Windows\\System32\\robocopy.exe"),
            &[
                "C:\\My Folder".to_string(),
                "D:\\.".to_string(),
                "/XD".to_string(),
                "Program Files".to_string(),
            ],
        );
        assert_eq!(
            cmd,
            "C:\\Windows\\System32\\robocopy.exe \"C:\\My Folder\" D:\\. /XD \"Program Files\""
        );
    }
}
