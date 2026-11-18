//! Concrete bash tool: the first real `Tool` implementation, executed by the
//! brain-tools stack behind the Inc 4 permission round-trip.
//! Cancellation and timeout terminate the owned process before returning, but
//! cannot roll back filesystem, network, or other command side effects that
//! occurred before termination.
use std::collections::HashMap;
use std::io::Read;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use brain_core::errors::BrainError;
use brain_core::extensibility::{
    ExecutionContext, ExecutionPolicy, ExecutionResult, Permission, Tool, ToolMetadata,
};

use brain_tools::sandbox::{default_sandbox_driver, EnforcementMode, SandboxDriver, SandboxPolicy};

/// Maximum payload size for combined output before truncation.
const OUTPUT_LIMIT_BYTES: usize = 32_768;
/// Maximum bytes buffered per stream (stdout/stderr) before applying backpressure and truncation.
const MAX_STREAM_BUFFER_BYTES: usize = 256 * 1024;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const TERMINATION_GRACE: Duration = Duration::from_millis(250);
const DEFAULT_BASH_TIMEOUT_MS: u64 = 30_000;
const READER_JOIN_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Default)]
pub struct BashTool;

impl BashTool {
    /// Executes a command with explicit sandbox driver and enforcement mode.
    pub fn execute_with_sandbox(
        &self,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
        driver: &dyn SandboxDriver,
        mode: EnforcementMode,
    ) -> Result<ExecutionResult, BrainError> {
        let command = match arguments.get("command").and_then(|v| v.as_str()) {
            Some(c) if !c.trim().is_empty() => c.to_string(),
            _ => {
                return Err(BrainError::Internal {
                    message: "bash tool requires a non-empty string 'command' argument".to_string(),
                })
            }
        };

        // SEC-03: Apply sandbox policy confinement and prepare command
        let policy = SandboxPolicy::builder(&context.working_dir)
            .with_system_defaults()
            .with_sensitive_defaults()
            .with_mode(mode)
            .build();

        let (mut process, sandbox_activation) =
            driver.prepare_command("/bin/bash", &["-c", &command], &policy)?;

        process
            .current_dir(&context.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // The child becomes leader of a dedicated process group before it
        // executes bash. On Unix this lets cancellation terminate the bash
        // command and descendants it starts without touching the daemon's
        // process group.
        #[cfg(unix)]
        process.process_group(0);

        let started_at = Instant::now();
        let mut child = process.spawn().map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("Seatbelt activation failed")
                || err_str.contains("Landlock")
                || err_str.contains("prctl")
                || err_str.contains("sandbox_init")
                || err_str.contains("sandbox_apply")
                || err_str.contains("failed to initialize sandbox")
            {
                BrainError::Authorization {
                    message: format!("Sandbox activation failure (fail-closed): {e}"),
                }
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                BrainError::Authorization {
                    message: format!("Command execution blocked by sandbox policy: {e}"),
                }
            } else {
                BrainError::Internal {
                    message: format!("failed to spawn /bin/bash: {e}"),
                }
            }
        })?;
        let child_pid = child.id();
        let stdout = child.stdout.take().ok_or_else(|| BrainError::Internal {
            message: "bash child stdout pipe was unavailable".to_string(),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| BrainError::Internal {
            message: "bash child stderr pipe was unavailable".to_string(),
        })?;

        // Draining both pipes concurrently with bounded buffering avoids runaway
        // memory usage while preventing detached child processes from hanging joins.
        let stdout_reader = spawn_bounded_reader(stdout);
        let stderr_reader = spawn_bounded_reader(stderr);

        let effective_deadline = context
            .deadline
            .unwrap_or_else(|| started_at + Duration::from_millis(bash_timeout_ms()));

        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| BrainError::Internal {
                message: format!("failed while waiting for bash process {child_pid}: {e}"),
            })? {
                break status;
            }

            // Observe normal completion first. If it happened immediately
            // before cancellation or timeout, normal completion wins.
            if context.cancellation.is_cancelled() {
                terminate_and_reap(&mut child, child_pid)?;
                let _ = drain_readers(stdout_reader, stderr_reader);
                return Err(BrainError::Cancelled {
                    message: "bash process was cancelled and reaped".to_string(),
                });
            }

            if Instant::now() >= effective_deadline {
                let elapsed_ms = started_at.elapsed().as_millis() as u64;
                terminate_and_reap(&mut child, child_pid)?;
                let _ = drain_readers(stdout_reader, stderr_reader);
                return Err(BrainError::Timeout {
                    elapsed_ms,
                    message: "bash process exceeded its execution deadline and was reaped"
                        .to_string(),
                });
            }

            thread::sleep(PROCESS_POLL_INTERVAL);
        };
        let (stdout, stderr) = drain_readers(stdout_reader, stderr_reader)?;

        // Ensure detached descendants in the process group do not survive and leak
        #[cfg(unix)]
        cleanup_surviving_process_group(child_pid);

        // stdout first, stderr appended after a newline separator when present,
        // UTF-8 lossy, truncated per spec §4.1.
        let mut text = String::from_utf8_lossy(&stdout).into_owned();
        if !stderr.is_empty() {
            if !text.ends_with('\n') && !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&String::from_utf8_lossy(&stderr));
        }
        if text.len() > OUTPUT_LIMIT_BYTES {
            let mut cut = OUTPUT_LIMIT_BYTES;
            while cut > 0 && !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            text.push('…');
            text.push_str("[truncated]");
        }

        let exit_code = status.code().unwrap_or(-1);

        // SEC-03: Distinguish sandbox activation failure from runtime filesystem access denial.
        // If the sandbox framework itself failed to initialize/apply confinement (e.g. sandbox-exec
        // failed with EX_OSERR 71, or pre_exec activation failed), execution must fail closed
        // under Enforced mode rather than reporting that the command executed and failed.
        let is_activation_failure = (exit_code == 71
            && (text.contains("sandbox_apply:")
                || text.contains("sandbox-exec:")
                || text.contains("sandbox initialization failed")
                || text.contains("failed to initialize sandbox")))
            || text.contains("Seatbelt activation failed");
        if is_activation_failure && mode == EnforcementMode::Enforced {
            return Err(BrainError::Authorization {
                message: format!("Sandbox activation failure (fail-closed): {}", text.trim()),
            });
        }
        Ok(ExecutionResult::new(serde_json::json!({
            "output": text,
            "exit_code": exit_code,
            "is_error": !status.success(),
            "sandbox": {
                "driver": sandbox_activation.driver_name,
                "activated": sandbox_activation.activated,
                "mode": sandbox_activation.mode,
                "workspace_root": sandbox_activation.workspace_root,
            }
        })))
    }
}

impl Tool for BashTool {
    fn metadata(&self) -> &ToolMetadata {
        // Delegate to the inherent static accessor; do NOT write
        // `&self.metadata()` here — that recurses into this very method.
        Self::meta()
    }

    fn execute(
        &self,
        context: &ExecutionContext,
        arguments: &HashMap<String, serde_json::Value>,
    ) -> Result<ExecutionResult, BrainError> {
        let driver = default_sandbox_driver();
        let mode = EnforcementMode::from_env();
        self.execute_with_sandbox(context, arguments, driver.as_ref(), mode)
    }
}

pub struct StreamBuffer {
    receiver: Receiver<std::io::Result<()>>,
    buffer: Arc<Mutex<Vec<u8>>>,
    stop_flag: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

#[cfg(unix)]
fn spawn_bounded_reader<R: Read + AsRawFd + Send + 'static>(mut stream: R) -> StreamBuffer {
    let (tx, rx) = channel();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buf_clone = buffer.clone();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();
    let raw_fd = stream.as_raw_fd();

    // Set non-blocking mode on the read end so read(2) never blocks indefinitely
    let flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFL) };
    if flags >= 0 {
        unsafe { libc::fcntl(raw_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    }

    let handle = thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }

            let mut pfd = libc::pollfd {
                fd: raw_fd,
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            };
            let poll_res = unsafe { libc::poll(&mut pfd, 1, 50) };
            if poll_res < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                let _ = tx.send(Err(err));
                return;
            }
            if poll_res == 0 {
                // Poll timed out (50ms) without data; recheck stop flag
                continue;
            }

            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let mut lock = buf_clone.lock().unwrap();
                    let remaining = MAX_STREAM_BUFFER_BYTES.saturating_sub(lock.len());
                    let to_take = remaining.min(n);
                    if to_take > 0 {
                        lock.extend_from_slice(&chunk[..to_take]);
                    }
                    if lock.len() >= MAX_STREAM_BUFFER_BYTES {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
        let _ = tx.send(Ok(()));
    });

    StreamBuffer {
        receiver: rx,
        buffer,
        stop_flag,
        handle,
    }
}

#[cfg(not(unix))]
fn spawn_bounded_reader<R: Read + Send + 'static>(mut stream: R) -> StreamBuffer {
    let (tx, rx) = channel();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buf_clone = buffer.clone();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_clone = stop_flag.clone();

    let handle = thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        loop {
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    let mut lock = buf_clone.lock().unwrap();
                    let remaining = MAX_STREAM_BUFFER_BYTES.saturating_sub(lock.len());
                    let to_take = remaining.min(n);
                    if to_take > 0 {
                        lock.extend_from_slice(&chunk[..to_take]);
                    }
                    if lock.len() >= MAX_STREAM_BUFFER_BYTES {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    let _ = tx.send(Err(e));
                    return;
                }
            }
        }
        let _ = tx.send(Ok(()));
    });

    StreamBuffer {
        receiver: rx,
        buffer,
        stop_flag,
        handle,
    }
}

pub fn read_bounded_stream(stream: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    stream
        .take(MAX_STREAM_BUFFER_BYTES as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn drain_reader_with_timeout(
    reader: StreamBuffer,
    stream_name: &'static str,
) -> Result<Vec<u8>, BrainError> {
    match reader.receiver.recv_timeout(READER_JOIN_TIMEOUT) {
        Ok(Ok(())) => {
            let _ = reader.handle.join();
            let bytes = reader.buffer.lock().unwrap().clone();
            Ok(bytes)
        }
        Ok(Err(e)) => {
            reader.stop_flag.store(true, Ordering::Relaxed);
            let _ = reader.handle.join();
            Err(BrainError::Internal {
                message: format!("failed to read bash {stream_name}: {e}"),
            })
        }
        Err(RecvTimeoutError::Timeout) => {
            tracing::warn!(
                "bash {stream_name} reader timed out after {:?}; detached child likely keeping pipe open",
                READER_JOIN_TIMEOUT
            );
            reader.stop_flag.store(true, Ordering::Relaxed);
            #[cfg(unix)]
            let _ = reader.handle.join();
            let bytes = reader.buffer.lock().unwrap().clone();
            Ok(bytes)
        }
        Err(RecvTimeoutError::Disconnected) => {
            reader.stop_flag.store(true, Ordering::Relaxed);
            #[cfg(unix)]
            let _ = reader.handle.join();
            let bytes = reader.buffer.lock().unwrap().clone();
            Ok(bytes)
        }
    }
}

fn drain_readers(
    stdout_reader: StreamBuffer,
    stderr_reader: StreamBuffer,
) -> Result<(Vec<u8>, Vec<u8>), BrainError> {
    let stdout = drain_reader_with_timeout(stdout_reader, "stdout")?;
    let stderr = drain_reader_with_timeout(stderr_reader, "stderr")?;
    Ok((stdout, stderr))
}

#[cfg(unix)]
fn cleanup_surviving_process_group(child_pid: u32) {
    if let Ok(true) = process_group_exists(child_pid) {
        let _ = unsafe { libc::kill(-(child_pid as libc::pid_t), libc::SIGTERM) };
        let deadline = Instant::now() + Duration::from_millis(50);
        while Instant::now() < deadline {
            if let Ok(false) = process_group_exists(child_pid) {
                return;
            }
            thread::sleep(PROCESS_POLL_INTERVAL);
        }
        if let Ok(true) = process_group_exists(child_pid) {
            let _ = unsafe { libc::kill(-(child_pid as libc::pid_t), libc::SIGKILL) };
        }
    }
}

fn terminate_and_reap(child: &mut Child, child_pid: u32) -> Result<(), BrainError> {
    request_termination(child, child_pid, false)?;

    let grace_deadline = Instant::now() + TERMINATION_GRACE;
    loop {
        let child_exited = child
            .try_wait()
            .map_err(|e| BrainError::Internal {
                message: format!("failed while waiting for bash process {child_pid}: {e}"),
            })?
            .is_some();
        if child_exited && !process_group_exists(child_pid)? {
            return Ok(());
        }
        if Instant::now() >= grace_deadline {
            break;
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }

    request_termination(child, child_pid, true)?;
    let force_deadline = Instant::now() + TERMINATION_GRACE;
    loop {
        let child_exited = child
            .try_wait()
            .map_err(|e| BrainError::Internal {
                message: format!("failed while waiting for bash process {child_pid}: {e}"),
            })?
            .is_some();
        if child_exited && !process_group_exists(child_pid)? {
            return Ok(());
        }
        if Instant::now() >= force_deadline {
            return Err(BrainError::Internal {
                message: format!(
                    "bash process group {child_pid} survived SIGKILL; refusing to report cancellation"
                ),
            });
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

#[cfg(unix)]
fn request_termination(child: &mut Child, child_pid: u32, force: bool) -> Result<(), BrainError> {
    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };
    let result = unsafe { libc::kill(-(child_pid as libc::pid_t), signal) };
    if result == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        // A command may deliberately leave the process group Brain created.
        // Fall back to the direct child so the executor still owns and reaps
        // that process; descendants that escape the group are documented as
        // an operating-system limitation rather than being silently ignored.
        if child.try_wait().map_err(|wait_error| BrainError::Internal {
            message: format!(
                "failed while checking bash process {child_pid} after its process group disappeared: {wait_error}"
            ),
        })?.is_some() {
            return Ok(());
        }
        return child.kill().map_err(|kill_error| BrainError::Internal {
            message: format!(
                "bash process group {child_pid} was absent and direct child termination failed: {kill_error}"
            ),
        });
    }
    Err(BrainError::Internal {
        message: format!(
            "failed to send {} to bash process group {child_pid}: {error}",
            if force { "SIGKILL" } else { "SIGTERM" }
        ),
    })
}

#[cfg(unix)]
fn process_group_exists(child_pid: u32) -> Result<bool, BrainError> {
    let result = unsafe { libc::kill(-(child_pid as libc::pid_t), 0) };
    if result == 0 {
        return Ok(true);
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    if error.raw_os_error() == Some(libc::EPERM) {
        return Ok(true);
    }
    Err(BrainError::Internal {
        message: format!("failed to inspect bash process group {child_pid}: {error}"),
    })
}

#[cfg(not(unix))]
fn request_termination(child: &mut Child, _child_pid: u32, _force: bool) -> Result<(), BrainError> {
    child.kill().map_err(|e| BrainError::Internal {
        message: format!("failed to terminate bash child process: {e}"),
    })
}

#[cfg(not(unix))]
fn process_group_exists(_child_pid: u32) -> Result<bool, BrainError> {
    Ok(false)
}

/// Keeps the production shell timeout fixed unless an explicit debug/mock E2E
/// fixture asks for a shorter deadline. This is deliberately not a user-facing
/// configuration surface: release builds and non-mock daemon processes always
/// retain `DEFAULT_BASH_TIMEOUT_MS`.
fn bash_timeout_ms_for_test_environment(
    e2e_test_mode: Option<&str>,
    has_mock_script: bool,
    requested_timeout_ms: Option<&str>,
) -> u64 {
    if !cfg!(debug_assertions) || e2e_test_mode != Some("1") || !has_mock_script {
        return DEFAULT_BASH_TIMEOUT_MS;
    }

    requested_timeout_ms
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|timeout_ms| *timeout_ms > 0)
        .unwrap_or(DEFAULT_BASH_TIMEOUT_MS)
}

fn bash_timeout_ms() -> u64 {
    let e2e_test_mode = std::env::var("BRAIN_E2E_TEST_MODE").ok();
    let requested_timeout_ms = std::env::var("BRAIN_TEST_BASH_TIMEOUT_MS").ok();
    bash_timeout_ms_for_test_environment(
        e2e_test_mode.as_deref(),
        std::env::var_os("BRAIN_MOCK_SCRIPTED_RESPONSES").is_some(),
        requested_timeout_ms.as_deref(),
    )
}

impl BashTool {
    fn meta() -> &'static ToolMetadata {
        use std::sync::OnceLock;
        static META: OnceLock<ToolMetadata> = OnceLock::new();
        META.get_or_init(|| ToolMetadata {
            name: "bash".to_string(),
            description:
                "Executes a shell command with /bin/bash -c in the daemon working directory."
                    .to_string(),
            usage: "bash {\"command\": \"<shell command>\"}".to_string(),
            version: "0.1.0".to_string(),
            author: "brain".to_string(),
            required_permissions: vec![Permission::Shell],
            execution_policy: ExecutionPolicy {
                timeout_ms: bash_timeout_ms(),
            },
            supports_streaming: false,
            is_idempotent: false,
            causes_side_effects: true,
            input_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute."
                    }
                },
                "required": ["command"]
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use brain_tools::{BlockingToolRunner, CancellationTokenImpl, PermissionManager, ToolExecutor};

    fn ctx() -> ExecutionContext {
        ExecutionContext {
            session_id: brain_domain::SessionId::new(),
            working_dir: std::env::temp_dir(),
            cancellation: Arc::new(brain_tools::CancellationTokenImpl::default()),
            deadline: None,
        }
    }

    fn args(command: &str) -> HashMap<String, serde_json::Value> {
        HashMap::from([("command".to_string(), serde_json::json!(command))])
    }

    #[test]
    fn echoes_stdout_with_zero_exit() {
        let result = BashTool.execute(&ctx(), &args("echo hello-inc5")).unwrap();
        let v = result.value();
        assert_eq!(v["is_error"], serde_json::json!(false));
        assert_eq!(v["exit_code"], serde_json::json!(0));
        assert!(v["output"].as_str().unwrap().contains("hello-inc5"));
    }

    #[test]
    fn non_zero_exit_is_a_result_not_an_error() {
        let result = BashTool.execute(&ctx(), &args("exit 3")).unwrap();
        let v = result.value();
        assert_eq!(v["is_error"], serde_json::json!(true));
        assert_eq!(v["exit_code"], serde_json::json!(3));
    }

    #[test]
    fn stderr_is_appended_after_stdout() {
        let result = BashTool
            .execute(&ctx(), &args("echo out; echo err 1>&2"))
            .unwrap();
        let v = result.value();
        assert!(v["output"].as_str().unwrap().contains("out"));
        assert!(v["output"].as_str().unwrap().contains("err"));
    }

    #[test]
    fn missing_or_empty_command_is_an_err() {
        assert!(BashTool.execute(&ctx(), &HashMap::new()).is_err());
        assert!(BashTool.execute(&ctx(), &args("   ")).is_err());
    }

    #[test]
    fn oversized_output_is_truncated_with_marker() {
        // Generate output inside Bash so this fixture needs no device access.
        let command = format!("printf '%0{}d' 0", OUTPUT_LIMIT_BYTES + 1024);
        let result = BashTool.execute(&ctx(), &args(&command)).unwrap();
        let out = result.value()["output"].as_str().unwrap();
        assert!(out.len() <= OUTPUT_LIMIT_BYTES + 16);
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn runaway_stdout_output_is_bounded_and_truncated() {
        let started = std::time::Instant::now();
        let result = BashTool.execute(&ctx(), &args("while :; do printf '%04096d' 0; done"));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "execution took too long: {:?}",
            started.elapsed()
        );
        let res =
            result.expect("runaway stdout command must succeed with bounded truncated output");
        let out = res.value()["output"].as_str().unwrap();
        assert!(out.len() <= OUTPUT_LIMIT_BYTES + 32);
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn runaway_stderr_output_is_bounded_and_truncated() {
        let started = std::time::Instant::now();
        let result = BashTool.execute(&ctx(), &args("while :; do printf '%04096d' 0 >&2; done"));
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "execution took too long: {:?}",
            started.elapsed()
        );
        let res =
            result.expect("runaway stderr command must succeed with bounded truncated output");
        let out = res.value()["output"].as_str().unwrap();
        assert!(out.len() <= OUTPUT_LIMIT_BYTES + 32);
        assert!(out.ends_with("…[truncated]"));
    }

    #[test]
    fn detached_grandchild_process_does_not_deadlock_reader_threads() {
        let started = std::time::Instant::now();
        // Spawns a background grandchild that deliberately inherits the pipe descriptors without /dev/null redirection.
        // drain_reader_with_timeout must break out after READER_JOIN_TIMEOUT (500ms) rather than deadlocking.
        let result = BashTool
            .execute(&ctx(), &args("echo 'parent-done'; (sleep 5 &)"))
            .unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "detached grandchild hung executor for {:?}",
            started.elapsed()
        );
        let out = result.value()["output"].as_str().unwrap();
        assert!(out.contains("parent-done"));
    }

    #[cfg(unix)]
    #[test]
    fn test_repeated_executions_with_detached_grandchildren_bounded_threads_and_subprocesses() {
        let temp = test_dir("repeated-grandchildren");
        let pids_file = temp.join("grandchildren.pids");
        let initial_thread_count = get_current_thread_count();
        let initial_rss = get_current_rss_bytes();

        for i in 0..5 {
            let cmd = format!(
                "echo iter-{i}; (sleep 30 & echo $! >> {})",
                pids_file.display()
            );
            let started = Instant::now();
            let result = BashTool.execute(&ctx(), &args(&cmd)).unwrap();
            assert!(
                started.elapsed() < Duration::from_secs(3),
                "iteration {i} took too long: {:?}",
                started.elapsed()
            );
            let out = result.value()["output"].as_str().unwrap();
            assert!(out.contains(&format!("iter-{i}")));
        }

        // Verify all grandchild processes were terminated and reaped
        if let Ok(content) = std::fs::read_to_string(&pids_file) {
            for line in content.lines() {
                if let Ok(pid) = line.trim().parse::<libc::pid_t>() {
                    let alive = unsafe { libc::kill(pid, 0) == 0 };
                    assert!(!alive, "grandchild process {pid} survived cleanup!");
                }
            }
        }

        // Verify thread count remains bounded
        let final_thread_count = get_current_thread_count();
        assert!(
            final_thread_count <= initial_thread_count + 2,
            "thread count grew unbounded from {initial_thread_count} to {final_thread_count}"
        );

        // Verify memory resident set size (RSS) remains bounded
        let final_rss = get_current_rss_bytes();
        if initial_rss > 0 && final_rss > 0 {
            let growth = final_rss.saturating_sub(initial_rss);
            assert!(
                growth < 30 * 1024 * 1024,
                "memory RSS grew excessively by {growth} bytes (initial: {initial_rss}, final: {final_rss})"
            );
        }

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn test_reader_threads_do_not_block_indefinitely_on_inherited_pipes() {
        let temp = test_dir("reader-threads-timeout");
        let pid_file = temp.join("child.pid");
        let initial_thread_count = get_current_thread_count();

        let cmd = format!(
            "echo 'start'; (sleep 30 & echo $! > {}); echo 'done'",
            pid_file.display()
        );
        let started = Instant::now();
        let result = BashTool.execute(&ctx(), &args(&cmd)).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "execution took too long: {:?}",
            started.elapsed()
        );
        let out = result.value()["output"].as_str().unwrap();
        assert!(out.contains("start"));
        assert!(out.contains("done"));

        // Wait briefly for OS thread scheduling settlement
        thread::sleep(Duration::from_millis(100));

        let current_threads = get_current_thread_count();
        assert!(
            current_threads <= initial_thread_count + 1,
            "reader threads were left blocked! initial: {initial_thread_count}, current: {current_threads}"
        );

        // Verify detached grandchild process in pid_file was terminated
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<libc::pid_t>() {
                let alive = unsafe { libc::kill(pid, 0) == 0 };
                assert!(
                    !alive,
                    "detached grandchild process {pid} survived cleanup!"
                );
            }
        }

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn metadata_requests_shell_permission() {
        let meta = BashTool::meta();
        assert_eq!(meta.name, "bash");
        assert!(meta.required_permissions.contains(&Permission::Shell));
        assert_eq!(meta.execution_policy.timeout_ms, 30_000);
        // Inc 7: the advertised schema mirrors execute()'s actual contract —
        // one non-empty string `command`, mandatory.
        let schema = meta
            .input_schema
            .as_ref()
            .expect("bash advertises a schema");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], serde_json::json!(["command"]));
        assert_eq!(schema["properties"]["command"]["type"], "string");
    }

    #[test]
    fn e2e_timeout_override_requires_explicit_mock_test_mode() {
        assert_eq!(
            bash_timeout_ms_for_test_environment(Some("1"), true, Some("600")),
            600
        );
        assert_eq!(
            bash_timeout_ms_for_test_environment(None, true, Some("600")),
            30_000
        );
        assert_eq!(
            bash_timeout_ms_for_test_environment(Some("0"), true, Some("600")),
            30_000
        );
        assert_eq!(
            bash_timeout_ms_for_test_environment(Some("1"), false, Some("600")),
            30_000
        );
        assert_eq!(
            bash_timeout_ms_for_test_environment(Some("1"), true, Some("invalid")),
            30_000
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_terminates_the_owned_shell_process() {
        let temp = test_dir("cancellation");
        let pid_path = temp.join("child.pid");
        let cancellation = Arc::new(CancellationTokenImpl::new());
        let context = ExecutionContext {
            session_id: brain_domain::SessionId::new(),
            working_dir: temp.clone(),
            cancellation: cancellation.clone(),
            deadline: None,
        };
        let command = format!("echo $$ > {}; exec sleep 60", pid_path.display());
        let args = args(&command);
        let permissions = PermissionManager::new();
        permissions.grant(Permission::Shell);
        let executor = ToolExecutor::new(Arc::new(BlockingToolRunner));

        let execution = tokio::spawn(async move {
            executor
                .execute(Arc::new(BashTool), &context, &permissions, &args)
                .await
        });

        let pid = wait_for_pid(&pid_path).await;
        cancellation.cancel();
        let result = execution.await.unwrap();
        assert!(matches!(result, Err(BrainError::Cancelled { .. })));

        assert_process_exited(pid).await;
        assert!(!process_group_exists(pid as u32).unwrap());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn deadline_terminates_the_owned_shell_process() {
        let temp = test_dir("deadline");
        let pid_path = temp.join("child.pid");
        let context = ExecutionContext {
            session_id: brain_domain::SessionId::new(),
            working_dir: temp.clone(),
            cancellation: Arc::new(CancellationTokenImpl::new()),
            deadline: Some(std::time::Instant::now() + Duration::from_millis(150)),
        };
        let command = format!("echo $$ > {}; exec sleep 60", pid_path.display());
        let args = args(&command);
        let permissions = PermissionManager::new();
        permissions.grant(Permission::Shell);
        let executor = ToolExecutor::new(Arc::new(BlockingToolRunner));

        let execution = tokio::spawn(async move {
            executor
                .execute(Arc::new(BashTool), &context, &permissions, &args)
                .await
        });

        let pid = wait_for_pid(&pid_path).await;
        let result = execution.await.unwrap();
        assert!(matches!(result, Err(BrainError::Timeout { .. })));
        assert_process_exited(pid).await;
        assert!(!process_group_exists(pid as u32).unwrap());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_terminates_shell_descendants_in_its_process_group() {
        let temp = test_dir("descendants");
        let pid_path = temp.join("children.pid");
        let cancellation = Arc::new(CancellationTokenImpl::new());
        let context = ExecutionContext {
            session_id: brain_domain::SessionId::new(),
            working_dir: temp.clone(),
            cancellation: cancellation.clone(),
            deadline: None,
        };
        let command = format!(
            "sleep 60 & child=$!; echo \"$$ $child\" > {}; wait",
            pid_path.display()
        );
        let args = args(&command);
        let permissions = PermissionManager::new();
        permissions.grant(Permission::Shell);
        let executor = ToolExecutor::new(Arc::new(BlockingToolRunner));

        let execution = tokio::spawn(async move {
            executor
                .execute(Arc::new(BashTool), &context, &permissions, &args)
                .await
        });

        let pids = wait_for_pids(&pid_path, 2).await;
        cancellation.cancel();
        let result = execution.await.unwrap();
        assert!(matches!(result, Err(BrainError::Cancelled { .. })));
        for pid in &pids {
            assert_process_exited(*pid).await;
        }
        assert!(!process_group_exists(pids[0] as u32).unwrap());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[tokio::test]
    async fn executor_preserves_normal_stdout_stderr_and_exit_status() {
        let permissions = PermissionManager::new();
        permissions.grant(Permission::Shell);
        let executor = ToolExecutor::new(Arc::new(BlockingToolRunner));
        let result = executor
            .execute(
                Arc::new(BashTool),
                &ctx(),
                &permissions,
                &args("printf normal-out; printf normal-err 1>&2; exit 7"),
            )
            .await
            .unwrap();

        assert_eq!(result.value()["exit_code"], serde_json::json!(7));
        assert_eq!(result.value()["is_error"], serde_json::json!(true));
        let output = result.value()["output"].as_str().unwrap();
        assert!(output.contains("normal-out"));
        assert!(output.contains("normal-err"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn completion_and_cancellation_races_leave_no_child_process() {
        for attempt in 0..12 {
            let temp = test_dir("race");
            let pid_path = temp.join("child.pid");
            let cancellation = Arc::new(CancellationTokenImpl::new());
            let context = ExecutionContext {
                session_id: brain_domain::SessionId::new(),
                working_dir: temp.clone(),
                cancellation: cancellation.clone(),
                deadline: None,
            };
            let args = args(&format!(
                "echo $$ > {}; exec sleep 0.03",
                pid_path.display()
            ));
            let permissions = PermissionManager::new();
            permissions.grant(Permission::Shell);
            let executor = ToolExecutor::new(Arc::new(BlockingToolRunner));
            let execution = tokio::spawn(async move {
                executor
                    .execute(Arc::new(BashTool), &context, &permissions, &args)
                    .await
            });

            let pid = wait_for_pid(&pid_path).await;
            if attempt % 2 == 0 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            } else {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            cancellation.cancel();

            let result = execution.await.unwrap();
            assert!(
                result.is_ok() || matches!(result, Err(BrainError::Cancelled { .. })),
                "race returned an unexpected terminal result: {result:?}"
            );
            assert_process_exited(pid).await;
            let _ = std::fs::remove_dir_all(&temp);
        }
    }

    #[cfg(unix)]
    async fn wait_for_pid(path: &std::path::Path) -> libc::pid_t {
        wait_for_pids(path, 1).await[0]
    }

    #[cfg(unix)]
    async fn wait_for_pids(path: &std::path::Path, expected: usize) -> Vec<libc::pid_t> {
        for _ in 0..100 {
            if let Ok(contents) = std::fs::read_to_string(path) {
                let pids: Vec<libc::pid_t> = contents
                    .split_whitespace()
                    .map(|pid| pid.parse().unwrap())
                    .collect();
                if pids.len() == expected {
                    return pids;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!(
            "shell process did not publish {expected} PIDs at {}",
            path.display()
        );
    }

    #[cfg(unix)]
    async fn wait_for_process_exit(pid: libc::pid_t) -> bool {
        for _ in 0..50 {
            let alive = unsafe { libc::kill(pid, 0) == 0 };
            if !alive {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    #[cfg(unix)]
    async fn assert_process_exited(pid: libc::pid_t) {
        let exited = wait_for_process_exit(pid).await;
        if !exited {
            // Keep the regression tests from leaking a process if a future
            // change regresses lifecycle management.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            let _ = wait_for_process_exit(pid).await;
        }
        assert!(exited, "bash process {pid} is still running");
    }

    #[cfg(unix)]
    fn test_dir(label: &str) -> std::path::PathBuf {
        let temp =
            std::env::temp_dir().join(format!("brain-bash-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp).unwrap();
        temp
    }

    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    fn get_current_thread_count() -> usize {
        unsafe {
            let mut count: libc::mach_msg_type_number_t = 0;
            let mut threads: libc::thread_act_array_t = std::ptr::null_mut();
            let kr = libc::task_threads(libc::mach_task_self(), &mut threads, &mut count);
            if kr == libc::KERN_SUCCESS {
                let n = count as usize;
                libc::vm_deallocate(
                    libc::mach_task_self(),
                    threads as libc::vm_address_t,
                    (count as usize * std::mem::size_of::<libc::thread_act_t>()) as libc::vm_size_t,
                );
                n
            } else {
                1
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn get_current_thread_count() -> usize {
        std::fs::read_dir("/proc/self/task")
            .map(|it| it.count())
            .unwrap_or(1)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn get_current_thread_count() -> usize {
        1
    }

    #[cfg(target_os = "macos")]
    #[allow(deprecated)]
    fn get_current_rss_bytes() -> usize {
        unsafe {
            let mut info: libc::mach_task_basic_info = std::mem::zeroed();
            let mut count = (std::mem::size_of::<libc::mach_task_basic_info>()
                / std::mem::size_of::<libc::natural_t>())
                as libc::mach_msg_type_number_t;
            let kr = libc::task_info(
                libc::mach_task_self(),
                libc::MACH_TASK_BASIC_INFO,
                &mut info as *mut _ as *mut libc::integer_t,
                &mut count,
            );
            if kr == libc::KERN_SUCCESS {
                info.resident_size as usize
            } else {
                0
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn get_current_rss_bytes() -> usize {
        std::fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| {
                s.split_whitespace()
                    .nth(1)
                    .and_then(|pages| pages.parse::<usize>().ok())
            })
            .map(|pages| pages * 4096)
            .unwrap_or(0)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn get_current_rss_bytes() -> usize {
        0
    }
}
