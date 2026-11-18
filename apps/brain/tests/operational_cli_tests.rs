#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "brain-operational-cli-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self { path }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct FixtureDaemon {
    socket_path: PathBuf,
    request: Receiver<serde_json::Value>,
    thread: Option<thread::JoinHandle<()>>,
}

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

fn short_socket_path(label: &str) -> PathBuf {
    let sequence = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bcli-{label}-{}-{sequence}.sock",
        std::process::id()
    ))
}

impl FixtureDaemon {
    fn start(socket_path: PathBuf, response: serde_json::Value) -> Self {
        let listener = UnixListener::bind(&socket_path).expect("bind fixture daemon socket");
        let (request_tx, request) = mpsc::channel();
        let thread = thread::spawn(move || {
            loop {
                let (stream, _) = listener.accept().expect("accept fixture client");
                let mut reader = BufReader::new(stream.try_clone().expect("clone fixture stream"));
                let mut request_line = String::new();
                let bytes_read = reader
                    .read_line(&mut request_line)
                    .expect("read fixture request");
                if bytes_read == 0 {
                    // `brain query` and `brain ingest` preflight the socket with a
                    // connection-only probe. Keep waiting for their real request.
                    continue;
                }
                let request_json = serde_json::from_str(request_line.trim())
                    .expect("fixture received JSON request");
                request_tx
                    .send(request_json)
                    .expect("record fixture request");

                let mut stream = stream;
                writeln!(stream, "{}", response).expect("write fixture response");
                stream.flush().expect("flush fixture response");
                break;
            }
        });
        Self {
            socket_path,
            request,
            thread: Some(thread),
        }
    }

    fn received_request(&self) -> serde_json::Value {
        self.request
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("fixture daemon should receive one request")
    }
}

impl Drop for FixtureDaemon {
    fn drop(&mut self) {
        if let Some(thread) = self.thread.take() {
            thread.join().expect("fixture daemon thread should finish");
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}

fn brain_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_brain"))
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_reports_effective_overrides_independently_of_current_directory() {
    let temp = TemporaryDirectory::new("config");
    let config_dir = temp.path.join("config-root");
    let socket_path = temp.path.join("overridden.sock");
    let database_path = temp.path.join("runtime.sqlite");
    let pid_path = temp.path.join("brain.pid");
    let log_path = temp.path.join("brain.log");
    let other_cwd = temp.path.join("other-cwd");
    fs::create_dir_all(&other_cwd).expect("create alternate cwd");

    let mut root_command = brain_command();
    root_command
        .arg("config")
        .env("BRAIN_CONFIG_DIR", &config_dir)
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_DB_PATH", &database_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_LOG_PATH", &log_path)
        .env("BRAIN_HEALTH_PORT", "19081");
    let root_output = root_command.output().expect("run brain config from root");
    assert_success(&root_output);
    let root_stdout = String::from_utf8(root_output.stdout).expect("config stdout is UTF-8");

    let mut other_command = brain_command();
    other_command
        .arg("config")
        .current_dir(&other_cwd)
        .env("BRAIN_CONFIG_DIR", &config_dir)
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_DB_PATH", &database_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_LOG_PATH", &log_path)
        .env("BRAIN_HEALTH_PORT", "19081");
    let other_output = other_command
        .output()
        .expect("run brain config from alternate cwd");
    assert_success(&other_output);
    let other_stdout = String::from_utf8(other_output.stdout).expect("config stdout is UTF-8");

    for expected in [
        config_dir.display().to_string(),
        socket_path.display().to_string(),
        database_path.display().to_string(),
        pid_path.display().to_string(),
        log_path.display().to_string(),
        "19081".to_string(),
    ] {
        assert!(
            root_stdout.contains(&expected),
            "missing {expected:?} in {root_stdout}"
        );
    }
    assert_eq!(
        root_stdout, other_stdout,
        "config resolution must not depend on cwd"
    );
}

#[test]
fn config_directory_derives_default_runtime_paths_before_explicit_overrides() {
    let temp = TemporaryDirectory::new("config-directory-defaults");
    let config_dir = temp.path.join("config-root");

    let output = brain_command()
        .arg("config")
        .env("BRAIN_CONFIG_DIR", &config_dir)
        .env_remove("BRAIN_SOCKET_PATH")
        .env_remove("BRAIN_DB_PATH")
        .env_remove("BRAIN_PID_PATH")
        .env_remove("BRAIN_LOG_PATH")
        .output()
        .expect("run brain config with config-directory override only");
    assert_success(&output);
    let stdout = String::from_utf8(output.stdout).expect("config stdout is UTF-8");
    for expected in [
        config_dir.display().to_string(),
        config_dir.join("daemon.sock").display().to_string(),
        config_dir.join("brain_runtime.db").display().to_string(),
        config_dir.join("daemon.pid").display().to_string(),
        config_dir.join("daemon.log").display().to_string(),
    ] {
        assert!(
            stdout.contains(&expected),
            "missing {expected:?} in {stdout}"
        );
    }
}

#[test]
fn version_commands_report_the_same_cargo_package_version() {
    let flag = brain_command()
        .arg("--version")
        .output()
        .expect("run brain --version");
    let subcommand = brain_command()
        .arg("version")
        .output()
        .expect("run brain version");
    assert_success(&flag);
    assert_success(&subcommand);

    let flag_stdout = String::from_utf8(flag.stdout).expect("version stdout is UTF-8");
    let subcommand_stdout = String::from_utf8(subcommand.stdout).expect("version stdout is UTF-8");
    assert_eq!(
        flag_stdout.split_whitespace().last(),
        subcommand_stdout.split_whitespace().last(),
        "--version and version must identify the same Brain build"
    );
    assert_eq!(
        subcommand_stdout.trim(),
        format!("brain version {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn query_uses_the_current_versioned_search_action() {
    let _temp = TemporaryDirectory::new("query");
    let socket_path = short_socket_path("query");
    let daemon = FixtureDaemon::start(
        socket_path.clone(),
        serde_json::json!({
            "version": "1.0",
            "type": "Response",
            "id": 1,
            "status": "success",
            "body": "[]"
        }),
    );

    let output = brain_command()
        .arg("query")
        .arg("find the current protocol")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .output()
        .expect("run brain query");
    assert_success(&output);

    let request = daemon.received_request();
    assert_eq!(request["version"], "1.0");
    assert_eq!(request["type"], "Request");
    assert_eq!(request["action"], "v1/search");
    let body: serde_json::Value =
        serde_json::from_str(request["body"].as_str().expect("versioned body string"))
            .expect("search body is JSON");
    assert_eq!(body["text"], "find the current protocol");
}

#[test]
fn ingest_uses_the_current_versioned_ingest_action() {
    let _temp = TemporaryDirectory::new("ingest");
    let socket_path = short_socket_path("ingest");
    let daemon = FixtureDaemon::start(
        socket_path.clone(),
        serde_json::json!({
            "version": "1.0",
            "type": "Response",
            "id": 1,
            "status": "success",
            "body": "{\"status\":\"ok\",\"processed\":true,\"event_id\":\"event-1\"}"
        }),
    );

    let output = brain_command()
        .arg("ingest")
        .arg("remember the versioned request")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .output()
        .expect("run brain ingest");
    assert_success(&output);

    let request = daemon.received_request();
    assert_eq!(request["version"], "1.0");
    assert_eq!(request["type"], "Request");
    assert_eq!(request["action"], "v1/ingest");
    let body: serde_json::Value =
        serde_json::from_str(request["body"].as_str().expect("versioned body string"))
            .expect("ingest body is JSON");
    assert_eq!(body["event_model_version"], "1.0");
    assert_eq!(body["event"]["event_type"], "text");
    assert_eq!(body["event"]["content"], "remember the versioned request");
}

#[test]
fn health_uses_a_versioned_status_request_and_reports_daemon_health() {
    let _temp = TemporaryDirectory::new("health");
    let socket_path = short_socket_path("health");
    let daemon = FixtureDaemon::start(
        socket_path.clone(),
        serde_json::json!({
            "version": "1.0",
            "type": "Response",
            "id": 1,
            "status": "success",
            "body": "{\"uptime_secs\":1,\"storage_backend\":\"sqlite\",\"active_event_subscribers\":0,\"health\":\"healthy\"}"
        }),
    );

    let output = brain_command()
        .arg("health")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .output()
        .expect("run brain health");
    assert_success(&output);
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("OK"),
        "health must report the daemon response"
    );

    let request = daemon.received_request();
    assert_eq!(request["action"], "v1/status");
}

#[test]
fn unavailable_daemon_returns_a_nonzero_exit_status() {
    let temp = TemporaryDirectory::new("unavailable");
    let output = brain_command()
        .arg("health")
        .env("BRAIN_SOCKET_PATH", temp.path.join("missing.sock"))
        .output()
        .expect("run brain health against unavailable daemon");
    assert!(
        !output.status.success(),
        "unavailable daemon must not be reported as a successful health check"
    );
}

#[test]
fn versioned_daemon_errors_produce_nonzero_cli_exits() {
    let _temp = TemporaryDirectory::new("backend-error");
    let socket_path = short_socket_path("backend-error");
    let daemon = FixtureDaemon::start(
        socket_path.clone(),
        serde_json::json!({
            "version": "1.0",
            "type": "Error",
            "id": 1,
            "status": "error",
            "body": "controlled backend failure"
        }),
    );

    let output = brain_command()
        .arg("query")
        .arg("trigger a backend error")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .output()
        .expect("run brain query against failing fixture daemon");
    assert!(
        !output.status.success(),
        "daemon errors must not be reported as success"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("controlled backend failure"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(daemon.received_request()["action"], "v1/search");
}
