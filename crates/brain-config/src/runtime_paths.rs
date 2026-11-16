//! Operational runtime path resolution shared by the CLI and daemon.

use std::path::{Path, PathBuf};

/// The effective filesystem and listener locations for one Brain runtime.
///
/// Values are resolved with the same precedence everywhere: an explicit
/// environment override wins, otherwise the path is derived from
/// `BRAIN_CONFIG_DIR`, and finally the platform default (`$HOME/.brain`, or
/// `/tmp/.brain` when `HOME` is unavailable) is used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    /// Root directory for Brain's local runtime state and configuration.
    pub config_dir: PathBuf,
    /// Unix-domain socket used by local Brain clients.
    pub socket_path: PathBuf,
    /// PID file used by the daemon lifecycle commands.
    pub pid_path: PathBuf,
    /// Daemon log file location.
    pub log_path: PathBuf,
    /// SQLite database used by the Brain runtime.
    pub database_path: PathBuf,
    /// TCP port exposed by the daemon's HTTP health endpoint.
    pub health_port: String,
}

/// Resolves operational paths without creating files or directories.
///
/// The resolution precedence for individual paths is: their explicit
/// environment variable (`BRAIN_SOCKET_PATH`, `BRAIN_PID_PATH`,
/// `BRAIN_LOG_PATH`, or `BRAIN_DB_PATH`), then `BRAIN_CONFIG_DIR`, then the
/// default runtime directory. `BRAIN_HEALTH_PORT` similarly overrides `8080`.
pub fn resolve_runtime_paths() -> RuntimePaths {
    let config_dir = environment_path("BRAIN_CONFIG_DIR").unwrap_or_else(default_config_dir);
    let socket_path =
        environment_path("BRAIN_SOCKET_PATH").unwrap_or_else(|| config_dir.join("daemon.sock"));
    let pid_path =
        environment_path("BRAIN_PID_PATH").unwrap_or_else(|| config_dir.join("daemon.pid"));
    let log_path =
        environment_path("BRAIN_LOG_PATH").unwrap_or_else(|| config_dir.join("daemon.log"));
    let database_path =
        environment_path("BRAIN_DB_PATH").unwrap_or_else(|| config_dir.join("brain_runtime.db"));
    let health_port = std::env::var("BRAIN_HEALTH_PORT").unwrap_or_else(|_| "8080".to_string());

    RuntimePaths {
        config_dir,
        socket_path,
        pid_path,
        log_path,
        database_path,
        health_port,
    }
}

/// Ensures that the parent directories required for a daemon runtime exist.
pub fn ensure_runtime_directories(paths: &RuntimePaths) -> std::io::Result<()> {
    std::fs::create_dir_all(&paths.config_dir)?;
    create_parent_if_present(&paths.socket_path)?;
    create_parent_if_present(&paths.pid_path)?;
    create_parent_if_present(&paths.log_path)?;
    create_parent_if_present(&paths.database_path)
}

fn environment_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn default_config_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".brain")
}

fn create_parent_if_present(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_paths_keep_database_and_socket_under_the_config_directory_by_default() {
        let paths = RuntimePaths {
            config_dir: PathBuf::from("/tmp/brain"),
            socket_path: PathBuf::from("/tmp/brain/daemon.sock"),
            pid_path: PathBuf::from("/tmp/brain/daemon.pid"),
            log_path: PathBuf::from("/tmp/brain/daemon.log"),
            database_path: PathBuf::from("/tmp/brain/brain_runtime.db"),
            health_port: "8080".to_string(),
        };

        assert_eq!(paths.socket_path.parent(), Some(Path::new("/tmp/brain")));
        assert_eq!(paths.database_path.parent(), Some(Path::new("/tmp/brain")));
    }
}
