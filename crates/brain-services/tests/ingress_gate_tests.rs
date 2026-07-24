use brain_services::distributed::*;
use brain_services::runtime::*;
use rusqlite::Connection;

#[test]
fn test_ingress_gate_rejects_unhealthy_or_stale_heartbeat() {
    let conn = Connection::open_in_memory().unwrap();
    let repo = SqliteExecutionRepository::new(conn);
    repo.init_schema().unwrap();

    let gate = CoordinatorIngressGate::new(repo);

    let unhealthy_hb = WorkerHeartbeat {
        worker_id: "worker-1".to_string(),
        timestamp: 1000,
        active_leases: vec![],
        status: WorkerStatus {
            current_load: 0.1,
            available_resources: Resources {
                cpu_cores: 4,
                memory_bytes: 8000,
                gpu_count: 0,
                custom_resources: std::collections::HashMap::new(),
            },
            active_lease_count: 0,
            is_healthy: false,
        },
    };

    assert!(gate.process_heartbeat(&unhealthy_hb).is_err());
}
