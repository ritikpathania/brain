use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BrainPaths {
    pub socket_path: PathBuf,
    pub pid_path: PathBuf,
    pub log_path: PathBuf,
    pub config_dir: PathBuf,
}

pub fn resolve_paths() -> BrainPaths {
    let config_dir = if let Ok(dir) = std::env::var("BRAIN_CONFIG_DIR") {
        PathBuf::from(dir)
    } else {
        let mut path = if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
        } else {
            PathBuf::from("/tmp")
        };
        path.push(".brain");
        path
    };
    let _ = fs::create_dir_all(&config_dir);

    let socket_path = std::env::var("BRAIN_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("daemon.sock"));

    let pid_path = std::env::var("BRAIN_PID_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("daemon.pid"));

    let log_path = std::env::var("BRAIN_LOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("daemon.log"));

    BrainPaths {
        socket_path,
        pid_path,
        log_path,
        config_dir,
    }
}
