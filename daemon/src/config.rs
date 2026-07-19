use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BrainPaths {
    pub socket_path: PathBuf,
    pub db_path: PathBuf,
    pub analytics_db_path: PathBuf,
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

    let db_path = std::env::var("BRAIN_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("memory.db"));

    let analytics_db_path = std::env::var("BRAIN_ANALYTICS_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("analytics.duckdb"));

    let pid_path = std::env::var("BRAIN_PID_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("daemon.pid"));

    let log_path = std::env::var("BRAIN_LOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir.join("daemon.log"));

    BrainPaths {
        socket_path,
        db_path,
        analytics_db_path,
        pid_path,
        log_path,
        config_dir,
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PluginConfig {
    pub active_embedding_provider: String,
    pub active_llm_provider: String,
    pub active_retrieval_algorithm: String,
    pub active_ranking_strategy: String,
    pub active_storage_backend: String,
    pub active_memory_extractor: String,
    pub active_exporter: String,
    #[serde(default)]
    pub enable_reflection: bool,
    #[serde(default = "default_kpp_mode")]
    pub kpp_mode: String,
}

fn default_kpp_mode() -> String {
    "shadow".to_string()
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            active_embedding_provider: "noop".to_string(),
            active_llm_provider: "noop".to_string(),
            active_retrieval_algorithm: "fuzzy".to_string(),
            active_ranking_strategy: "default".to_string(),
            active_storage_backend: "sqlite".to_string(),
            active_memory_extractor: "python-default".to_string(),
            active_exporter: "duckdb".to_string(),
            enable_reflection: false,
            kpp_mode: "shadow".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompatibilityConfig {
    pub legacy_enabled: bool,
}

impl CompatibilityConfig {
    pub fn resolve() -> Self {
        let legacy_enabled = std::env::var("BRAIN_DISABLE_LEGACY_COMPAT")
            .map(|val| val != "1")
            .unwrap_or(true);
        Self { legacy_enabled }
    }
}
