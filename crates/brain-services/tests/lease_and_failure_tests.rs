use brain_services::coordinator::*;
use brain_services::runtime::*;

#[test]
fn test_failure_detector_detects_worker_lost_and_recovery() {
    let mut detector = FailureDetector::new(5); // 5s timeout

    let w1 = "worker-1".to_string();
    detector.record_heartbeat(w1.clone(), 1000);

    // Within timeout -> healthy
    assert!(detector.check_health(w1.clone(), 1004).is_none());

    // Past timeout -> WorkerLost
    let lost_ev = detector.check_health(w1.clone(), 1006).unwrap();
    assert!(matches!(lost_ev, InternalEvent::WorkerLost { .. }));

    // Heartbeat resumes -> WorkerRecovered
    detector.record_heartbeat(w1.clone(), 1010);
    let rec_ev = detector.check_health(w1, 1010).unwrap();
    assert!(matches!(rec_ev, InternalEvent::WorkerRecovered { .. }));
}
