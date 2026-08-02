//! Sandboxed-by-default shell command execution with bounded output.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

use crate::engine::cancellation::CancellationToken;
use crate::engine::matcher::FileItem;

const MAX_COMMAND_LENGTH: usize = 16_384;
const MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_TIMEOUT_MS: u32 = 10 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct CommandExecution {
    pub success: bool,
    pub cancelled: bool,
    pub error: Option<String>,
    pub command: String,
    pub exit_code: Option<i32>,
    pub output: String,
    pub duration_ms: u32,
    pub timed_out: bool,
}

pub async fn execute_shell_command_cancellable(
    command: &str,
    file: &FileItem,
    timeout_ms: u32,
    cancellation: Option<CancellationToken>,
) -> CommandExecution {
    let started = Instant::now();
    let command = command.trim();
    if command.is_empty() || command.len() > MAX_COMMAND_LENGTH {
        return failed_execution(command, started, "COMMAND_INVALID", false, false);
    }
    if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
        return failed_execution(command, started, "COMMAND_TIMEOUT_INVALID", false, false);
    }

    let expanded = expand_placeholders(command, file);
    let mut process = platform_shell(&expanded);
    let folder = Path::new(&file.path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    process
        .env("FILE_PATH", &file.path)
        .env("FILE_NAME", &file.name)
        .env("FOLDER", &folder);
    if let Some(parent) = Path::new(&file.path).parent() {
        process.current_dir(parent);
    }
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        process.process_group(0);
    }

    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            return failed_execution(
                command,
                started,
                &format!("COMMAND_UNAVAILABLE: {error}"),
                false,
                false,
            )
        }
    };
    let process_id = child.id();
    let stdout = child
        .stdout
        .take()
        .map(|stream| tokio::spawn(read_bounded(stream, MAX_OUTPUT_BYTES / 2)));
    let stderr = child
        .stderr
        .take()
        .map(|stream| tokio::spawn(read_bounded(stream, MAX_OUTPUT_BYTES / 2)));

    let deadline = started + Duration::from_millis(u64::from(timeout_ms));
    let mut cancelled = false;
    let mut timed_out = false;
    let status = loop {
        if cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            cancelled = true;
            terminate_process_tree(&mut child, process_id).await;
            break child.wait().await.ok();
        }
        if Instant::now() >= deadline {
            timed_out = true;
            terminate_process_tree(&mut child, process_id).await;
            break child.wait().await.ok();
        }
        match tokio::time::timeout(Duration::from_millis(50), child.wait()).await {
            Ok(result) => break result.ok(),
            Err(_) => continue,
        }
    };

    let (stdout, stdout_truncated) = join_output(stdout).await;
    let (stderr, stderr_truncated) = join_output(stderr).await;
    let mut output = String::from_utf8_lossy(&stdout).into_owned();
    if !stderr.is_empty() {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&String::from_utf8_lossy(&stderr));
    }
    if stdout_truncated || stderr_truncated {
        if !output.ends_with('\n') && !output.is_empty() {
            output.push('\n');
        }
        output.push_str("…[output truncated]");
    }

    let exit_code = status.and_then(|status| status.code());
    let (success, error) = if cancelled {
        (false, Some("COMMAND_CANCELLED".to_string()))
    } else if timed_out {
        (false, Some("COMMAND_TIMED_OUT".to_string()))
    } else if status.is_some_and(|status| status.success()) {
        (true, None)
    } else {
        (
            false,
            Some(match exit_code {
                Some(code) => format!("COMMAND_EXIT: {code}"),
                None => "COMMAND_TERMINATED".to_string(),
            }),
        )
    };

    CommandExecution {
        success,
        cancelled,
        error,
        command: command.to_string(),
        exit_code,
        output,
        duration_ms: elapsed_ms(started),
        timed_out,
    }
}

fn expand_placeholders(command: &str, file: &FileItem) -> String {
    let folder = Path::new(&file.path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    command
        .replace("{filePath}", &shell_quote(&file.path))
        .replace("{fileName}", &shell_quote(&file.name))
        .replace("{folder}", &shell_quote(&folder))
}

#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(windows)]
fn shell_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn platform_shell(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut process = Command::new("cmd.exe");
        process.arg("/D").arg("/S").arg("/C").arg(command);
        process
    }
    #[cfg(not(windows))]
    {
        let mut process = Command::new("/bin/sh");
        process.arg("-c").arg(command);
        process
    }
}

async fn read_bounded(mut stream: impl AsyncRead + Unpin, limit: usize) -> (Vec<u8>, bool) {
    let mut retained = Vec::with_capacity(limit.min(64 * 1024));
    let mut buffer = [0u8; 16 * 1024];
    let mut truncated = false;
    loop {
        let read = match stream.read(&mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let available = limit.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(available)]);
        truncated |= read > available;
    }
    (retained, truncated)
}

async fn join_output(task: Option<tokio::task::JoinHandle<(Vec<u8>, bool)>>) -> (Vec<u8>, bool) {
    match task {
        Some(task) => task.await.unwrap_or_else(|_| (Vec::new(), false)),
        None => (Vec::new(), false),
    }
}

async fn terminate_process_tree(child: &mut tokio::process::Child, process_id: Option<u32>) {
    #[cfg(unix)]
    if let Some(process_id) = process_id {
        let _ = Command::new("/bin/kill")
            .arg("-KILL")
            .arg(format!("-{process_id}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    #[cfg(windows)]
    if let Some(process_id) = process_id {
        let _ = Command::new("taskkill.exe")
            .args(["/PID", &process_id.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
    let _ = child.start_kill();
}

fn failed_execution(
    command: &str,
    started: Instant,
    error: &str,
    cancelled: bool,
    timed_out: bool,
) -> CommandExecution {
    CommandExecution {
        success: false,
        cancelled,
        error: Some(error.to_string()),
        command: command.to_string(),
        exit_code: None,
        output: String::new(),
        duration_ms: elapsed_ms(started),
        timed_out,
    }
}

fn elapsed_ms(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_millis()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn file(path: String) -> FileItem {
        FileItem {
            name: Path::new(&path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            path,
            extension: ".txt".into(),
            size: 0,
            modified_at: Utc::now(),
            created_at: Utc::now(),
            identity: None,
        }
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn placeholder_values_cannot_inject_shell_syntax() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("a; printf INJECTED.txt");
        std::fs::write(&path, b"").unwrap();
        let execution = execute_shell_command_cancellable(
            "printf '%s' {fileName}",
            &file(path.to_string_lossy().into_owned()),
            2_000,
            None,
        )
        .await;
        assert!(execution.success);
        assert_eq!(execution.output, "a; printf INJECTED.txt");
    }
}
