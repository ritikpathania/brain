use brain_domain::entities::{Edge, RelationKind};
use brain_domain::identifiers::{EdgeId, NodeId};
use brain_domain::temporal::{
    Clock, RecencyPolicy, TemporalEdge, TemporalProjector, TemporalQuery, TemporalValidity,
    TemporalVisibility, TestClock, TimeInterval, TimeIntervalError, TimePoint,
};
use std::time::Duration;

#[test]
fn test_timepoint_opaque_construction_and_checked_ops() {
    let t = TimePoint::from_unix_seconds(1000);
    assert_eq!(t.unix_seconds(), 1000);

    let t_plus = t.checked_add(Duration::from_secs(50)).unwrap();
    assert_eq!(t_plus.unix_seconds(), 1050);

    let t_minus = t.checked_sub(Duration::from_secs(150)).unwrap();
    assert_eq!(t_minus.unix_seconds(), 850);

    // Assert ordering
    assert!(t_minus < t);
    assert!(t_plus > t);
}

#[test]
fn test_time_interval_invariants_and_error() {
    let start = TimePoint::from_unix_seconds(1000);
    let end = TimePoint::from_unix_seconds(2000);

    // Valid interval
    let val_int = TimeInterval::new(start, Some(end));
    assert!(val_int.is_ok());
    let interval = val_int.unwrap();
    assert_eq!(interval.start(), start);
    assert_eq!(interval.end(), Some(end));

    // Open-ended valid interval
    let open_int = TimeInterval::new(start, None);
    assert!(open_int.is_ok());

    // Invalid interval start > end
    let err_int = TimeInterval::new(end, Some(start));
    assert_eq!(
        err_int.err(),
        Some(TimeIntervalError::StartAfterEnd {
            start: end,
            end: start,
        })
    );
}

#[test]
fn test_time_interval_contains_overlaps_intersect() {
    let t10 = TimePoint::from_unix_seconds(10);
    let t20 = TimePoint::from_unix_seconds(20);
    let t30 = TimePoint::from_unix_seconds(30);
    let t40 = TimePoint::from_unix_seconds(40);

    let int_10_30 = TimeInterval::new(t10, Some(t30)).unwrap();
    let int_20_40 = TimeInterval::new(t20, Some(t40)).unwrap();
    let int_30_40 = TimeInterval::new(t30, Some(t40)).unwrap();

    // Contains
    assert!(int_10_30.contains(t10));
    assert!(int_10_30.contains(t20));
    assert!(!int_10_30.contains(t30)); // half-open [10, 30)
    assert!(!int_10_30.contains(t40));

    // Overlaps
    assert!(int_10_30.overlaps(&int_20_40)); // overlaps at [20, 30)
    assert!(!int_10_30.overlaps(&int_30_40)); // [10, 30) and [30, 40) do not overlap

    // Intersect
    let intersection = int_10_30.intersect(&int_20_40).unwrap();
    assert_eq!(intersection.start(), t20);
    assert_eq!(intersection.end(), Some(t30));

    assert_eq!(int_10_30.intersect(&int_30_40), None);
}

#[test]
fn test_temporal_validity_matching() {
    let t10 = TimePoint::from_unix_seconds(10);
    let t20 = TimePoint::from_unix_seconds(20);
    let t30 = TimePoint::from_unix_seconds(30);
    let t40 = TimePoint::from_unix_seconds(40);

    let int_10_20 = TimeInterval::new(t10, Some(t20)).unwrap();
    let int_30_40 = TimeInterval::new(t30, Some(t40)).unwrap();

    let validity = TemporalValidity::new(vec![int_10_20, int_30_40]);

    assert!(validity.is_valid_at(TimePoint::from_unix_seconds(15)));
    assert!(!validity.is_valid_at(TimePoint::from_unix_seconds(25)));
    assert!(validity.is_valid_at(TimePoint::from_unix_seconds(35)));

    // Intersects target interval
    let int_15_25 = TimeInterval::new(
        TimePoint::from_unix_seconds(15),
        Some(TimePoint::from_unix_seconds(25)),
    )
    .unwrap();
    assert!(validity.intersects_interval(&int_15_25));

    let int_22_28 = TimeInterval::new(
        TimePoint::from_unix_seconds(22),
        Some(TimePoint::from_unix_seconds(28)),
    )
    .unwrap();
    assert!(!validity.intersects_interval(&int_22_28));
}

#[test]
fn test_clock_mocking_and_advancement() {
    let clock = TestClock::new(500);
    assert_eq!(clock.now(), TimePoint::from_unix_seconds(500));

    clock.advance(150);
    assert_eq!(clock.now(), TimePoint::from_unix_seconds(650));
}

#[test]
fn test_recency_policy_decay_calculations() {
    let obs_time = TimePoint::from_unix_seconds(100);
    let ref_time_100 = TimePoint::from_unix_seconds(100);
    let ref_time_200 = TimePoint::from_unix_seconds(200);

    // Exponential Decay
    let exp_policy = RecencyPolicy::Exponential {
        half_life_secs: 100.0,
    };

    // Elapsed = 0, weight should be unchanged
    let w0 = exp_policy.compute_weight(1.0, obs_time, ref_time_100);
    assert!((w0 - 1.0).abs() < 1e-9);

    // Elapsed = half-life, weight should be exactly 0.5
    let w1 = exp_policy.compute_weight(1.0, obs_time, ref_time_200);
    assert!((w1 - 0.5).abs() < 1e-9);

    // Linear Decay
    let lin_policy = RecencyPolicy::Linear {
        horizon_secs: 200.0,
    };

    // Elapsed = 0, weight = 1.0
    let wl0 = lin_policy.compute_weight(1.0, obs_time, ref_time_100);
    assert!((wl0 - 1.0).abs() < 1e-9);

    // Elapsed = 100 (half of horizon 200), weight = 0.5
    let wl1 = lin_policy.compute_weight(1.0, obs_time, ref_time_200);
    assert!((wl1 - 0.5).abs() < 1e-9);

    // Elapsed beyond horizon, weight = 0.0
    let wl2 = lin_policy.compute_weight(1.0, obs_time, TimePoint::from_unix_seconds(350));
    assert_eq!(wl2, 0.0);
}

fn create_dummy_temporal_edge(
    source: NodeId,
    target: NodeId,
    validity_intervals: Vec<TimeInterval>,
    observed_sec: u64,
) -> TemporalEdge {
    let edge = Edge::new(source, target, RelationKind::Uses, 1.0);
    TemporalEdge {
        edge,
        validity: TemporalValidity::new(validity_intervals),
        observed_at: TimePoint::from_unix_seconds(observed_sec),
    }
}

#[test]
fn test_temporal_snapshot_visibility_projection() {
    let t10 = TimePoint::from_unix_seconds(10);
    let t20 = TimePoint::from_unix_seconds(20);
    let t30 = TimePoint::from_unix_seconds(30);

    let int_10_20 = TimeInterval::new(t10, Some(t20)).unwrap();
    let int_20_30 = TimeInterval::new(t20, Some(t30)).unwrap();

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();
    let node_e = NodeId::new();
    let node_f = NodeId::new();

    let edges = vec![
        // Edge A: validity [10, 20), observed at 5
        create_dummy_temporal_edge(node_a, node_b, vec![int_10_20], 5),
        // Edge B: validity [20, 30), observed at 15
        create_dummy_temporal_edge(node_c, node_d, vec![int_20_30], 15),
        // Edge C: validity [10, 30), observed at 25 (observed in the future relative to T=20)
        create_dummy_temporal_edge(
            node_e,
            node_f,
            vec![TimeInterval::new(t10, Some(t30)).unwrap()],
            25,
        ),
    ];

    // Current query at T = 15
    let query_current_15 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(15),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    let snap_current_15 = TemporalProjector::project(&edges, &query_current_15);
    // Edge A is valid at 15 and observed before 15.
    // Edge B is not valid at 15 (validity starts at 20).
    // Edge C is observed in the future (25 > 15).
    assert_eq!(snap_current_15.active_edge_ids.len(), 1);
    let edge_a_id = EdgeId::new(node_a, node_b, RelationKind::Uses.id());
    assert!(snap_current_15.active_edge_ids.contains(&edge_a_id));

    // Historical query at T = 20
    let query_hist_20 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(20),
        visibility: TemporalVisibility::Historical,
        recency_policy: RecencyPolicy::None,
    };
    let snap_hist_20 = TemporalProjector::project(&edges, &query_hist_20);
    // Edge A was valid in the past [10, 20) and observed at 5 <= 20. (Visible)
    // Edge B starts validity at 20 <= 20 and observed at 15 <= 20. (Visible)
    // Edge C is observed in the future (25 > 20). (Filtered out)
    assert_eq!(snap_hist_20.active_edge_ids.len(), 2);

    // Interval intersection query: looking at [15, 25) with reference T = 22
    let query_interval = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(22),
        visibility: TemporalVisibility::Interval(
            TimeInterval::new(
                TimePoint::from_unix_seconds(15),
                Some(TimePoint::from_unix_seconds(25)),
            )
            .unwrap(),
        ),
        recency_policy: RecencyPolicy::None,
    };
    let snap_interval = TemporalProjector::project(&edges, &query_interval);
    // Intersection limit [15, 22) (since reference is 22).
    // Edge A validity [10, 20) overlaps [15, 22) -> overlaps. Observed at 5 <= 22. (Visible)
    // Edge B validity [20, 30) overlaps [15, 22) -> overlaps. Observed at 15 <= 22. (Visible)
    // Edge C observed at 25 > 22. (Filtered out)
    assert_eq!(snap_interval.active_edge_ids.len(), 2);
}

#[test]
fn test_visibility_equivalence_and_determinism_invariants() {
    let t10 = TimePoint::from_unix_seconds(10);
    let t20 = TimePoint::from_unix_seconds(20);
    let t30 = TimePoint::from_unix_seconds(30);

    let node_a = NodeId::new();
    let node_b = NodeId::new();
    let node_c = NodeId::new();
    let node_d = NodeId::new();
    let node_e = NodeId::new();
    let node_f = NodeId::new();

    let edges = vec![
        create_dummy_temporal_edge(
            node_a,
            node_b,
            vec![TimeInterval::new(t10, Some(t20)).unwrap()],
            5,
        ),
        create_dummy_temporal_edge(
            node_c,
            node_d,
            vec![TimeInterval::new(t20, Some(t30)).unwrap()],
            15,
        ),
        create_dummy_temporal_edge(
            node_e,
            node_f,
            vec![TimeInterval::new(t10, Some(t30)).unwrap()],
            8,
        ),
    ];

    let all_edge_ids: std::collections::HashSet<EdgeId> = edges
        .iter()
        .map(|te| EdgeId::new(te.edge.source, te.edge.target, te.edge.relation.id()))
        .collect();

    // Invariant: Projection Idempotency & Determinism
    // Repeated projections must produce byte-for-byte identical snapshots.
    let query = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(15),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    let snap1 = TemporalProjector::project(&edges, &query);
    let snap2 = TemporalProjector::project(&edges, &query);
    assert_eq!(snap1, snap2);

    // Invariant: Snapshot Membership Invariant
    // visible_edge_ids ⊆ all_graph_edge_ids
    assert!(snap1.active_edge_ids.is_subset(&all_edge_ids));

    // Invariant: Visibility Equivalence Current(T) ⊆ Historical(T)
    // Any edge visible under Current at time T must also be visible under Historical at T.
    let reference_times = [5, 10, 15, 20, 25, 30];
    for &ref_sec in &reference_times {
        let t = TimePoint::from_unix_seconds(ref_sec);
        let q_current = TemporalQuery {
            reference_time: t,
            visibility: TemporalVisibility::Current,
            recency_policy: RecencyPolicy::None,
        };
        let q_historical = TemporalQuery {
            reference_time: t,
            visibility: TemporalVisibility::Historical,
            recency_policy: RecencyPolicy::None,
        };
        let snap_current = TemporalProjector::project(&edges, &q_current);
        let snap_historical = TemporalProjector::project(&edges, &q_historical);

        // Check subset inclusion: all elements in snap_current must exist in snap_historical
        assert!(snap_current
            .active_edge_ids
            .is_subset(&snap_historical.active_edge_ids));
    }

    // Invariant: Snapshot Monotonicity
    // If no validity transitions happen between T1 and T2, then snapshot(T1) == snapshot(T2)
    let q_11 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(11),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    let q_14 = TemporalQuery {
        reference_time: TimePoint::from_unix_seconds(14),
        visibility: TemporalVisibility::Current,
        recency_policy: RecencyPolicy::None,
    };
    let snap_11 = TemporalProjector::project(&edges, &q_11);
    let snap_14 = TemporalProjector::project(&edges, &q_14);
    assert_eq!(snap_11.active_edge_ids, snap_14.active_edge_ids);
}
