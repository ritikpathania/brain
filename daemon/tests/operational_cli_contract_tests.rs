#![cfg(unix)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "brain-daemon-operational-cli-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn daemon_config_reports_the_shared_effective_runtime_paths() {
    let temp = TemporaryDirectory::new();
    let config_dir = temp.0.join("config");
    let socket_path = temp.0.join("daemon.sock");
    let database_path = temp.0.join("runtime.sqlite");
    let pid_path = temp.0.join("daemon.pid");
    let log_path = temp.0.join("daemon.log");

    let output = Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("config")
        .env("BRAIN_CONFIG_DIR", &config_dir)
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_DB_PATH", &database_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_LOG_PATH", &log_path)
        .env("BRAIN_HEALTH_PORT", "19082")
        .output()
        .expect("run brain-daemon config");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("daemon config stdout is UTF-8");
    for expected in [
        config_dir.display().to_string(),
        socket_path.display().to_string(),
        database_path.display().to_string(),
        pid_path.display().to_string(),
        log_path.display().to_string(),
        "19082".to_string(),
    ] {
        assert!(
            stdout.contains(&expected),
            "missing {expected:?} in {stdout}"
        );
    }
}

#[test]
fn daemon_version_uses_its_cargo_package_metadata() {
    let output = Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("--version")
        .output()
        .expect("run brain-daemon --version");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("daemon version stdout is UTF-8")
            .trim(),
        format!("brain-daemon version {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn daemon_health_returns_nonzero_when_the_effective_health_endpoint_is_unavailable() {
    let temp = TemporaryDirectory::new();
    let health_port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("reserve a free health port")
        .local_addr()
        .expect("read reserved health address")
        .port();
    let output = Command::new(env!("CARGO_BIN_EXE_brain-daemon"))
        .arg("health")
        .env("BRAIN_CONFIG_DIR", &temp.0)
        .env("BRAIN_HEALTH_PORT", health_port.to_string())
        .output()
        .expect("run brain-daemon health");
    assert!(
        !output.status.success(),
        "unavailable daemon health must return nonzero"
    );
}
