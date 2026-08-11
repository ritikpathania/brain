use std::fs;
use std::path::Path;

#[test]
fn test_release_candidate_soak_report_metrics_gates() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let report_path = workspace_root.join("target/release_candidate_soak_report.json");

    assert!(
        report_path.exists(),
        "Soak report missing at {:?}",
        report_path
    );

    let content = fs::read_to_string(report_path).unwrap();
    assert!(
        content.contains("\"socket_health_rate\": 1.0"),
        "Socket health rate was not 100%"
    );
    assert!(
        content.contains("\"fd_delta\": 0"),
        "File descriptor leak detected in soak report"
    );
    assert!(
        content.contains("\"thread_delta\": 0"),
        "Thread leak detected in soak report"
    );
}
