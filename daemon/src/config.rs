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
    let mut config_dir = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home)
    } else {
        PathBuf::from("/tmp")
    };
    config_dir.push(".brain");
    let _ = fs::create_dir_all(&config_dir);

    BrainPaths {
        socket_path: config_dir.join("daemon.sock"),
        db_path: config_dir.join("memory.db"),
        analytics_db_path: config_dir.join("analytics.duckdb"),
        pid_path: config_dir.join("daemon.pid"),
        log_path: config_dir.join("daemon.log"),
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
        }
    }
}
