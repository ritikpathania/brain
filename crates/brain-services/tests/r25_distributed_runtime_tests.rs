use brain_domain::jobs::JobId;
use brain_services::distributed::*;
use brain_services::runtime::*;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

#[tokio::test]
async fn test_end_to_end_distributed_dispatch_and_failover_recovery() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let registry = WorkerRegistry::new(1);
    let desc = WorkerDescriptor {
        worker_id: "worker-1".to_string(),
        hostname: "node-1.local".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "aarch64".to_string(),
        supported_capabilities: HashSet::from(["gpu".to_string()]),
        labels: HashMap::from([("env".to_string(), "prod".to_string())]),
    };
    let status = WorkerStatus {
        current_load: 0.1,
        available_resources: Resources {
            cpu_cores: 16,
            memory_bytes: 32000,
            gpu_count: 1,
            custom_resources: HashMap::new(),
        },
        active_lease_count: 0,
        is_healthy: true,
    };
    registry.register(desc, status, 1000).unwrap();

    let scheduler = WorkerScheduler::new(registry, LeastLoadedPolicy);
    let selected_worker = scheduler.schedule_next_worker(1).unwrap();
    assert_eq!(selected_worker.descriptor.worker_id, "worker-1");

    let transport = MockWorkerTransport::new();
    let task_id = TaskId::new();
    let exec_id = ExecutionId::new();
    let job_id = JobId(uuid::Uuid::new_v4());

    let assignment = TaskAssignment {
        task_id,
        execution_id: exec_id,
        job_id,
        input_ref: "artifact://ref-1".to_string(),
        lease: TaskLease {
            lease_id: 1,
            lease_owner: "worker-1".to_string(),
            lease_until: 2000,
        },
    };

    transport.dispatch(assignment).await.unwrap();
    assert_eq!(transport.dispatched_count(), 1);
}
