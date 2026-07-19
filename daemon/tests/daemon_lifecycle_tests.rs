use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::net::UnixStream;

fn get_temp_dir() -> PathBuf {
    let uuid_str = uuid::Uuid::new_v4().to_string();
    // Keep socket path under /tmp and extremely short to avoid SUN_LEN (104 bytes) limits on macOS/Unix
    let path = PathBuf::from(format!("/tmp/bd-{}", &uuid_str[0..8]));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn get_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test]
async fn test_daemon_lifecycle_graceful_shutdown() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    // Start daemon in foreground using "run"
    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

    // Wait for the socket to be bound
    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");

    // Send SIGTERM to the daemon
    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    // Wait for the daemon to exit
    let status = tokio::time::timeout(Duration::from_secs(5), async { child.wait() })
        .await
        .expect("Daemon shutdown timed out")
        .expect("Failed to wait on daemon");

    assert!(status.success(), "Daemon did not exit cleanly");

    // Verify socket and pid files were cleaned up
    assert!(!socket_path.exists(), "Socket file was not cleaned up");
    assert!(!pid_path.exists(), "PID file was not cleaned up");

    // Cleanup temp dir
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_sigint_graceful_shutdown() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    // Start daemon in foreground using "run"
    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start daemon process");

    // Wait for the socket to be bound
    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket in time");

    // Send SIGINT to the daemon
    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGINT);
    }

    // Wait for the daemon to exit
    let status = tokio::time::timeout(Duration::from_secs(5), async { child.wait() })
        .await
        .expect("Daemon shutdown timed out")
        .expect("Failed to wait on daemon");

    assert!(status.success(), "Daemon did not exit cleanly on SIGINT");

    // Verify socket and pid files were cleaned up
    assert!(
        !socket_path.exists(),
        "Socket file was not cleaned up on SIGINT"
    );
    assert!(!pid_path.exists(), "PID file was not cleaned up on SIGINT");

    // Cleanup temp dir
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_repeated_signals() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .spawn()
        .expect("Failed to start daemon process");

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not bind socket");

    let pid = child.id() as i32;
    unsafe {
        // Send SIGTERM multiple times to verify idempotence
        libc::kill(pid, libc::SIGTERM);
        libc::kill(pid, libc::SIGTERM);
    }

    let status = tokio::time::timeout(Duration::from_secs(5), async { child.wait() })
        .await
        .expect("Daemon shutdown timed out on repeated SIGTERM")
        .expect("Failed to wait on daemon");

    assert!(status.success(), "Daemon failed on repeated SIGTERM");
    assert!(!socket_path.exists());
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_stale_socket_recovery() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    // Write a dummy stale socket file
    fs::write(&socket_path, "stale socket contents").unwrap();
    assert!(socket_path.exists());

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .spawn()
        .expect("Failed to start daemon process");

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready, "Daemon did not recover from stale socket");

    let pid = child.id() as i32;
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }
    let _ = child.wait();
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_double_start_prevention() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    // Start daemon 1
    let mut child1 = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .spawn()
        .unwrap();

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready);

    // Attempt to start daemon 2 on same socket
    let output2 = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .output()
        .unwrap();

    assert!(
        !output2.status.success(),
        "Second daemon instance should have failed to start"
    );

    // Stop daemon 1
    let pid1 = child1.id() as i32;
    unsafe {
        libc::kill(pid1, libc::SIGTERM);
    }
    let _ = child1.wait();
    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_interrupted_startup_cleanup() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    // Invalid path to force failure before listener init
    let analytics_db_path = PathBuf::from("/nonexistent/directory/analytics.db");

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", "/nonexistent/directory")
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(!status.success(), "Daemon should have failed to start");

    // Cleanup guard should delete the PID/socket files even on early exit
    assert!(
        !socket_path.exists(),
        "Socket file was not cleaned up on startup failure"
    );
    assert!(
        !pid_path.exists(),
        "PID file was not cleaned up on startup failure"
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_crash_during_worker_execution() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .spawn()
        .unwrap();

    let mut ready = false;
    for _ in 0..50 {
        if socket_path.exists() && UnixStream::connect(&socket_path).await.is_ok() {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(ready);

    // Connect to the socket and send an incomplete message to simulate active worker connection
    {
        let mut _stream = UnixStream::connect(&socket_path).await.unwrap();
        // Keep the stream open, then trigger SIGTERM
        let pid = child.id() as i32;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }

    let _ = child.wait();

    // Verify that the files were cleaned up successfully
    assert!(
        !socket_path.exists(),
        "Socket file was not cleaned up on worker connection close"
    );
    assert!(
        !pid_path.exists(),
        "PID file was not cleaned up on worker connection close"
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_panic_during_startup() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_TEST_PANIC_STARTUP", "1")
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(!status.success(), "Daemon should have failed (panicked)");

    assert!(
        !socket_path.exists(),
        "Socket file was not cleaned up on startup panic"
    );
    assert!(
        !pid_path.exists(),
        "PID file was not cleaned up on startup panic"
    );

    let _ = fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_daemon_lifecycle_panic_after_socket_creation() {
    let bin_path = env!("CARGO_BIN_EXE_brain-daemon");
    let test_dir = get_temp_dir();
    let socket_path = test_dir.join("brain.sock");
    let pid_path = test_dir.join("brain.pid");
    let db_path = test_dir.join("brain.db");
    let analytics_db_path = test_dir.join("analytics.db");

    let mut child = Command::new(bin_path)
        .arg("daemon")
        .arg("run")
        .env("BRAIN_SOCKET_PATH", &socket_path)
        .env("BRAIN_PID_PATH", &pid_path)
        .env("BRAIN_DB_PATH", &db_path)
        .env("BRAIN_ANALYTICS_DB_PATH", &analytics_db_path)
        .env("BRAIN_CONFIG_DIR", &test_dir)
        .env("BRAIN_HEALTH_PORT", get_free_port().to_string())
        .env("BRAIN_TEST_PANIC_BEFORE_SERVING", "1")
        .spawn()
        .unwrap();

    let status = child.wait().unwrap();
    assert!(!status.success(), "Daemon should have failed (panicked)");

    assert!(
        !socket_path.exists(),
        "Socket file was not cleaned up on post-bind panic"
    );
    assert!(
        !pid_path.exists(),
        "PID file was not cleaned up on post-bind panic"
    );

    let _ = fs::remove_dir_all(&test_dir);
}
