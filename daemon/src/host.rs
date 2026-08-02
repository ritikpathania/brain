use std::fs;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

use brain_services::{ApplicationRuntime, BrainRuntime};

use crate::config::{self, BrainPaths};
use crate::server::{start_health_server, start_uds_listener};
use crate::DaemonMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonState {
    Starting,
    Running,
    Draining,
    Stopped,
}

pub struct DaemonCleanupGuard {
    pid_path: Option<std::path::PathBuf>,
    socket_path: Option<std::path::PathBuf>,
}

impl DaemonCleanupGuard {
    pub fn new(pid_path: std::path::PathBuf, socket_path: std::path::PathBuf) -> Self {
        Self {
            pid_path: Some(pid_path),
            socket_path: Some(socket_path),
        }
    }

    pub fn disarm(&mut self) {
        self.pid_path = None;
        self.socket_path = None;
    }
}

impl Drop for DaemonCleanupGuard {
    fn drop(&mut self) {
        if let Some(path) = self.pid_path.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Failed to remove stale PID file: {}", e);
                }
            }
        }
        if let Some(path) = self.socket_path.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eprintln!("Failed to remove stale UDS socket file: {}", e);
                }
            }
        }
    }
}

pub fn is_pid_running(pid: i32) -> bool {
    unsafe {
        let res = libc::kill(pid, 0);
        res == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

pub struct DaemonHost;

impl DaemonHost {
    pub fn start() -> Result<(), Box<dyn std::error::Error>> {
        let paths = config::resolve_paths();
        if paths.pid_path.exists() {
            if let Ok(pid_str) = fs::read_to_string(&paths.pid_path) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    if is_pid_running(pid) {
                        println!("Daemon is already running (PID: {}).", pid);
                        return Ok(());
                    }
                }
            }
            let _ = fs::remove_file(&paths.pid_path);
        }

        let exe = std::env::current_exe()?;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&paths.log_path)?;

        println!("Starting brain daemon in background...");

        let child = std::process::Command::new(exe)
            .arg("daemon")
            .arg("run")
            .stdout(std::process::Stdio::from(log_file.try_clone()?))
            .stderr(std::process::Stdio::from(log_file))
            .spawn()?;

        let pid = child.id() as i32;
        fs::write(&paths.pid_path, pid.to_string())?;

        // Poll for socket readiness (up to 1s) so chained shell commands succeed deterministically
        for _ in 0..50 {
            if paths.socket_path.exists()
                && std::os::unix::net::UnixStream::connect(&paths.socket_path).is_ok()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        println!("Daemon started successfully (PID: {}).", pid);

        Ok(())
    }

    pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
        let paths = config::resolve_paths();
        if !paths.pid_path.exists() {
            println!("Daemon is not running (no PID file found).");
            return Ok(());
        }

        let pid_str = fs::read_to_string(&paths.pid_path)?;
        let pid = pid_str.trim().parse::<i32>()?;

        println!("Stopping daemon (PID: {})...", pid);
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }

        for _ in 0..50 {
            if !is_pid_running(pid) {
                println!("Daemon stopped.");
                let _ = fs::remove_file(&paths.pid_path);
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        println!("Daemon did not stop gracefully. Forcing exit...");
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        let _ = fs::remove_file(&paths.pid_path);
        Ok(())
    }

    pub fn status() -> Result<(), Box<dyn std::error::Error>> {
        let paths = config::resolve_paths();
        if !paths.pid_path.exists() {
            println!("Status: Stopped");
            return Ok(());
        }

        let pid_str = fs::read_to_string(&paths.pid_path)?;
        let pid = pid_str.trim().parse::<i32>()?;

        if is_pid_running(pid) {
            println!("Status: Running (PID: {})", pid);
        } else {
            println!("Status: Stale PID file (Process not running)");
        }
        Ok(())
    }

    pub async fn run_server(paths: BrainPaths) -> Result<(), Box<dyn std::error::Error>> {
        let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let use_json = std::env::var("LOG_FORMAT").as_deref() == Ok("json");

        crate::telemetry::init_subscriber(&log_level, use_json);

        info!(component = "main", "Starting brain Daemon...");

        let metrics = Arc::new(DaemonMetrics::new());
        let health_metrics = Arc::clone(&metrics);

        let mut cleanup_guard =
            DaemonCleanupGuard::new(paths.pid_path.clone(), paths.socket_path.clone());

        #[cfg(debug_assertions)]
        if std::env::var("BRAIN_TEST_PANIC_STARTUP").is_ok() {
            panic!("Simulating panic during startup");
        }

        let brain_runtime_db_path = paths.config_dir.join("brain_runtime.db");
        let brain_runtime = match BrainRuntime::new(brain_runtime_db_path.to_str().unwrap()) {
            Ok(rt) => {
                info!(
                    component = "runtime",
                    db_path = %brain_runtime_db_path.display(),
                    "BrainRuntime initialized"
                );
                Arc::new(rt)
            }
            Err(e) => {
                error!(
                    component = "runtime",
                    "Failed to initialize BrainRuntime: {}", e
                );
                return Err(e.into());
            }
        };

        let defaults_src = brain_config::loader::DefaultsSource;
        let config = brain_config::loader::resolve(&[Box::new(defaults_src)])?;
        let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        let app_runtime = Arc::new(
            ApplicationRuntime::builder()
                .with_config(config)
                .with_working_dir(working_dir)
                .with_brain_runtime(Arc::clone(&brain_runtime))
                .build()?,
        );

        app_runtime.start()?;

        info!(
            component = "runtime",
            db_path = %paths.config_dir.join("brain_runtime.db").display(),
            "ApplicationRuntime and BrainRuntime initialized via RuntimeBuilder"
        );

        let brain_app = Arc::new(brain_application::BrainApplication::new(Arc::clone(
            &brain_runtime,
        )));
        let request_dispatcher = Arc::new(brain_application::dispatcher::RequestDispatcher::new(
            Arc::clone(&brain_app),
        ));

        let dispatcher_ref = Arc::clone(&request_dispatcher);
        tokio::spawn(async move {
            start_health_server(health_metrics, dispatcher_ref).await;
        });

        let mut state = DaemonState::Starting;
        info!(component = "daemon", "Daemon state: {:?}", state);

        if paths.socket_path.exists() {
            match UnixStream::connect(&paths.socket_path).await {
                Ok(_) => {
                    error!(
                        component = "socket",
                        "UDS socket at '{}' is active. Another daemon is running. Aborting startup.",
                        paths.socket_path.display()
                    );
                    return Err("Daemon already running".into());
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    error!(
                        component = "socket",
                        "Permission denied checking socket status at '{}': {}. Aborting startup.",
                        paths.socket_path.display(),
                        e
                    );
                    return Err(e.into());
                }
                Err(e) => {
                    info!(
                        component = "socket",
                        "Stale or invalid file/socket detected at '{}' (Error: {}). Cleaning up...",
                        paths.socket_path.display(),
                        e
                    );
                    if let Err(err) = fs::remove_file(&paths.socket_path) {
                        if err.kind() != std::io::ErrorKind::NotFound {
                            warn!(
                                component = "socket",
                                "Failed to remove stale UDS file: {}", err
                            );
                        }
                    }
                }
            }
        }

        let listener = UnixListener::bind(&paths.socket_path)?;
        info!(component = "socket", socket_path = %paths.socket_path.display(), "Socket bound successfully");

        #[cfg(debug_assertions)]
        if std::env::var("BRAIN_TEST_PANIC_BEFORE_SERVING").is_ok() {
            panic!("Simulating panic after socket creation but before serving");
        }

        state = DaemonState::Running;
        info!(component = "daemon", "Daemon state: {:?}", state);

        let cancel_token = tokio_util::sync::CancellationToken::new();
        let cancel_trigger = cancel_token.clone();

        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

        tokio::spawn(async move {
            let reason = tokio::select! {
                _ = tokio::signal::ctrl_c() => "SIGINT",
                _ = sigterm.recv() => "SIGTERM",
            };
            info!(component = "shutdown", "Shutdown initiated ({})", reason);
            cancel_trigger.cancel();

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        info!(component = "shutdown", "Shutdown already in progress (SIGINT ignored)");
                    }
                    _ = sigterm.recv() => {
                        info!(component = "shutdown", "Shutdown already in progress (SIGTERM ignored)");
                    }
                }
            }
        });

        let listener_metrics = metrics.clone();
        let listener_dispatcher = Arc::clone(&request_dispatcher);
        let listener_app = Arc::clone(&brain_app);
        tokio::select! {
            _ = start_uds_listener(
                listener,
                listener_metrics,
                listener_dispatcher,
                listener_app,
            ) => {}
            _ = cancel_token.cancelled() => {
                state = DaemonState::Draining;
                info!(component = "daemon", "Daemon state: {:?}", state);
            }
        }

        info!(component = "shutdown", "Stopping accept loop");

        let start_drain = std::time::Instant::now();
        let mut active = metrics
            .active_workers
            .load(std::sync::atomic::Ordering::Relaxed);
        while active > 0 {
            info!(
                component = "shutdown",
                "Waiting for workers ({} active)", active
            );
            if start_drain.elapsed() >= std::time::Duration::from_secs(5) {
                warn!(
                    component = "shutdown",
                    "Draining timed out after 5s. Forcing exit."
                );
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            active = metrics
                .active_workers
                .load(std::sync::atomic::Ordering::Relaxed);
        }

        if active == 0 {
            info!(component = "shutdown", "Workers drained");
        }

        drop(request_dispatcher);
        drop(brain_app);

        if let Ok(runtime) = Arc::try_unwrap(brain_runtime) {
            match runtime.shutdown() {
                Ok(summary) => {
                    info!(
                        component = "runtime",
                        shutdown_ms = summary.duration.as_millis(),
                        "BrainRuntime shutdown complete"
                    );
                }
                Err(e) => {
                    error!(component = "runtime", error = %e, "BrainRuntime shutdown error");
                }
            }
        }

        state = DaemonState::Stopped;
        info!(component = "daemon", "Daemon state: {:?}", state);

        info!(component = "shutdown", "Removing socket");
        if let Some(path) = cleanup_guard.socket_path.take() {
            if let Err(e) = fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(
                        component = "shutdown",
                        "Failed to remove UDS socket file: {}", e
                    );
                }
            }
        }

        info!(component = "shutdown", "Removing PID");
        if let Some(path) = cleanup_guard.pid_path.take() {
            if let Err(e) = fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!(component = "shutdown", "Failed to remove PID file: {}", e);
                }
            }
        }

        cleanup_guard.disarm();
        info!(component = "shutdown", "Shutdown complete");

        Ok(())
    }
}
