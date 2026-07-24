use brain_services::distributed::*;

#[test]
fn test_least_loaded_scheduling_policy_with_candidate_view() {
    let policy = LeastLoadedPolicy;

    let desc1 = WorkerDescriptor {
        worker_id: "w1".to_string(),
        hostname: "node1".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status1 = WorkerStatus {
        current_load: 0.8,
        available_resources: Resources {
            cpu_cores: 4,
            memory_bytes: 8000,
            gpu_count: 0,
            custom_resources: std::collections::HashMap::new(),
        },
        active_lease_count: 4,
        is_healthy: true,
    };

    let desc2 = WorkerDescriptor {
        worker_id: "w2".to_string(),
        hostname: "node2".to_string(),
        protocol_version: 1,
        runtime_version: "1.0.0".to_string(),
        architecture: "x86_64".to_string(),
        supported_capabilities: std::collections::HashSet::new(),
        labels: std::collections::HashMap::new(),
    };
    let status2 = WorkerStatus {
        current_load: 0.2,
        available_resources: Resources {
            cpu_cores: 8,
            memory_bytes: 16000,
            gpu_count: 0,
            custom_resources: std::collections::HashMap::new(),
        },
        active_lease_count: 1,
        is_healthy: true,
    };

    let c1 = WorkerCandidate {
        descriptor: &desc1,
        status: &status1,
    };
    let c2 = WorkerCandidate {
        descriptor: &desc2,
        status: &status2,
    };

    let candidates = vec![c1, c2];
    let selected = policy.select_worker(1, &candidates).unwrap();
    assert_eq!(selected.descriptor.worker_id, "w2");
}
