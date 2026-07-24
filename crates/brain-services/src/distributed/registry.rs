#![allow(missing_docs)]

use crate::distributed::models::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("Incompatible protocol version {0}, expected {1}")]
    IncompatibleProtocol(u32, u32),
    #[error("Worker {0} not found")]
    WorkerNotFound(String),
}

#[derive(Debug, Clone)]
pub struct RegisteredWorker {
    pub descriptor: WorkerDescriptor,
    pub status: WorkerStatus,
    pub last_seen_timestamp: u64,
}

pub struct WorkerRegistry {
    expected_protocol_version: u32,
    workers: Arc<RwLock<HashMap<String, RegisteredWorker>>>,
}

impl WorkerRegistry {
    pub fn new(expected_protocol_version: u32) -> Self {
        Self {
            expected_protocol_version,
            workers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn register(&self, descriptor: WorkerDescriptor, status: WorkerStatus, timestamp: u64) -> Result<(), RegistryError> {
        if descriptor.protocol_version != self.expected_protocol_version {
            return Err(RegistryError::IncompatibleProtocol(
                descriptor.protocol_version,
                self.expected_protocol_version,
            ));
        }

        let id = descriptor.worker_id.clone();
        let entry = RegisteredWorker {
            descriptor,
            status,
            last_seen_timestamp: timestamp,
        };

        self.workers.write().insert(id, entry);
        Ok(())
    }

    pub fn get(&self, worker_id: &str) -> Option<RegisteredWorker> {
        self.workers.read().get(worker_id).cloned()
    }

    pub fn list_active(&self) -> Vec<RegisteredWorker> {
        self.workers.read().values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_worker_registry_registration_and_protocol_version() {
        let registry = WorkerRegistry::new(1); // Current protocol version = 1

        let descriptor = WorkerDescriptor {
            worker_id: "worker-1".to_string(),
            hostname: "node-1.local".to_string(),
            protocol_version: 1,
            runtime_version: "1.0.0".to_string(),
            architecture: "x86_64".to_string(),
            supported_capabilities: HashSet::from(["gpu".to_string()]),
            labels: HashMap::from([("region".to_string(), "us-east".to_string())]),
        };

        let status = WorkerStatus {
            current_load: 0.1,
            available_resources: Resources {
                cpu_cores: 8,
                memory_bytes: 16000,
                gpu_count: 1,
                custom_resources: HashMap::new(),
            },
            active_lease_count: 0,
            is_healthy: true,
        };

        assert!(registry.register(descriptor.clone(), status.clone(), 1000).is_ok());
        assert_eq!(registry.get("worker-1").unwrap().last_seen_timestamp, 1000);

        // Incompatible protocol version rejected
        let invalid = WorkerDescriptor {
            protocol_version: 99,
            ..descriptor
        };
        assert!(registry.register(invalid, status, 1000).is_err());
    }
}
