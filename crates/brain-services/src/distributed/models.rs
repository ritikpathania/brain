#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerDescriptor {
    pub worker_id: String,
    pub hostname: String,
    pub protocol_version: u32,
    pub runtime_version: String,
    pub architecture: String,
    pub supported_capabilities: HashSet<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub cpu_cores: u32,
    pub memory_bytes: u64,
    pub gpu_count: u32,
    pub custom_resources: HashMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub current_load: f32,
    pub available_resources: Resources,
    pub active_lease_count: u32,
    pub is_healthy: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerCandidate<'a> {
    pub descriptor: &'a WorkerDescriptor,
    pub status: &'a WorkerStatus,
}
