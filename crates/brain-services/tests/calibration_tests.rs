use brain_domain::retrieval::{
    CostHeuristics, HeuristicMetadata, HeuristicWeights, EstimatedCost,
    RetrievalExecutionReport, PlanningMetadata, RuntimeMetadata,
    LogicalRetrievalPlan, LogicalStep, PlanOptimizer
};
use brain_services::retrieval::calibration::CostCalibrator;

fn make_report(
    elapsed: u64,
    estimated: EstimatedCost,
    vector_active: bool,
    keyword_active: bool,
    expansion_active: bool,
    fusion_active: bool,
    ranking_active: bool,
) -> RetrievalExecutionReport {
    RetrievalExecutionReport {
        planning: PlanningMetadata {
            estimated_cost: estimated,
            planner_decisions: vec![],
            optimizer_decisions: vec![],
            heuristics_version: 1,
        },
        runtime: RuntimeMetadata {
            elapsed_microseconds: elapsed,
            candidates_produced: if vector_active || keyword_active { 100 } else { 0 },
            candidates_fused: if fusion_active { 50 } else { 0 },
            expansions_performed: if expansion_active { 10 } else { 0 },
            ranking_operations: if ranking_active { 20 } else { 0 },
        },
    }
}

#[test]
fn test_calibration_confidence_gating() {
    let initial = CostHeuristics::default();
    let calibrator = CostCalibrator::new(initial, 0.5, 3);

    // Initial version is 1
    assert_eq!(calibrator.active_heuristics().metadata.version, 1);

    // Est cost with vector retrieve
    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 0.0,
        expansion_cost: 0.0,
        fusion_cost: 0.0,
        ranking_cost: 0.0,
    };

    // Feed first report (1 observation)
    let rep1 = make_report(10000, est.clone(), true, false, false, false, false);
    let updated1 = calibrator.record_execution(&rep1);
    assert!(!updated1, "Should not update yet: observations = 1 < 3");
    assert_eq!(calibrator.active_heuristics().metadata.version, 1);

    // Feed second report (2 observations)
    let rep2 = make_report(12000, est.clone(), true, false, false, false, false);
    let updated2 = calibrator.record_execution(&rep2);
    assert!(!updated2, "Should not update yet: observations = 2 < 3");
    assert_eq!(calibrator.active_heuristics().metadata.version, 1);

    // Feed third report (3 observations -> trigger!)
    let rep3 = make_report(15000, est.clone(), true, false, false, false, false);
    let updated3 = calibrator.record_execution(&rep3);
    assert!(updated3, "Should update: observations = 3 >= 3");
    
    let active = calibrator.active_heuristics();
    assert_eq!(active.metadata.version, 2);
    
    // Check that vector weight has evolved from 10.0 toward observed latency
    assert!(active.weights.vector_weight > 10.0, "Vector weight should increase toward observed latency");
    assert_eq!(calibrator.history().len(), 1, "Should record 1 snapshot in history");
    assert_eq!(calibrator.history()[0].metadata.version, 1);
}

#[test]
fn test_calibration_purity_invariant() {
    let initial = CostHeuristics::default();
    // Gating set to 1 to allow immediate update on eligible reports
    let calibrator = CostCalibrator::new(initial, 0.5, 1);

    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 0.0,
        expansion_cost: 0.0,
        fusion_cost: 0.0,
        ranking_cost: 0.0,
    };

    // Report representing result cache hit (very low elapsed time, e.g., 20 microseconds)
    let hit_report = make_report(20, est.clone(), true, false, false, false, false);
    let updated = calibrator.record_execution(&hit_report);
    assert!(!updated, "Result cache hits must be ignored to prevent cost deflation");
    assert_eq!(calibrator.active_heuristics().metadata.version, 1);
}

#[test]
fn test_calibration_snapshot_history_cap() {
    let initial = CostHeuristics::default();
    let calibrator = CostCalibrator::new(initial, 0.5, 1);

    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 0.0,
        expansion_cost: 0.0,
        fusion_cost: 0.0,
        ranking_cost: 0.0,
    };

    // Trigger 15 sequential updates
    for i in 1..=15 {
        let rep = make_report(10000 + i * 1000, est.clone(), true, false, false, false, false);
        calibrator.record_execution(&rep);
    }

    let history = calibrator.history();
    assert!(history.len() <= 10, "History ring buffer must cap at 10 items");
}

#[test]
fn test_calibration_tolerance_gating() {
    let initial = CostHeuristics::default();
    let calibrator = CostCalibrator::new(initial, 0.5, 1);

    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 0.0,
        expansion_cost: 0.0,
        fusion_cost: 0.0,
        ranking_cost: 0.0,
    };

    // Feed report with latency matching current weight exactly
    let rep = make_report(10, est.clone(), true, false, false, false, false);
    let updated = calibrator.record_execution(&rep);
    assert!(!updated, "Should not update if change is below 1e-5 tolerance");
    assert_eq!(calibrator.active_heuristics().metadata.version, 1);
}

#[test]
fn test_calibration_determinism() {
    let initial1 = CostHeuristics::default();
    let calibrator1 = CostCalibrator::new(initial1, 0.3, 2);

    let initial2 = CostHeuristics::default();
    let calibrator2 = CostCalibrator::new(initial2, 0.3, 2);

    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 2.0,
        expansion_cost: 5.0,
        fusion_cost: 1.0,
        ranking_cost: 0.5,
    };

    let rep1 = make_report(15000, est.clone(), true, true, true, true, true);
    let rep2 = make_report(18000, est.clone(), true, true, true, true, true);

    calibrator1.record_execution(&rep1);
    calibrator1.record_execution(&rep2);

    calibrator2.record_execution(&rep1);
    calibrator2.record_execution(&rep2);

    let h1 = calibrator1.active_heuristics();
    let h2 = calibrator2.active_heuristics();

    assert_eq!(h1.metadata.version, h2.metadata.version);
    assert_eq!(h1.weights.vector_weight, h2.weights.vector_weight);
    assert_eq!(h1.weights.keyword_weight, h2.weights.keyword_weight);
    assert_eq!(h1.weights.expansion_weight, h2.weights.expansion_weight);
    assert_eq!(h1.weights.fusion_weight, h2.weights.fusion_weight);
    assert_eq!(h1.weights.ranking_weight, h2.weights.ranking_weight);
}

#[test]
fn test_calibration_monotonic_convergence() {
    let initial = CostHeuristics::default();
    let calibrator = CostCalibrator::new(initial, 0.2, 1);

    let est = EstimatedCost {
        vector_cost: 10.0,
        keyword_cost: 0.0,
        expansion_cost: 0.0,
        fusion_cost: 0.0,
        ranking_cost: 0.0,
    };

    let mut last_diff = f64::MAX;

    // Feed a stable latency distribution of exactly 5000 microseconds
    for _ in 0..15 {
        let rep = make_report(5000, est.clone(), true, false, false, false, false);
        calibrator.record_execution(&rep);
        
        let current_weight = calibrator.active_heuristics().weights.vector_weight;
        let diff = (current_weight - 5000.0).abs();
        
        // Assert variance/oscillation is strictly shrinking
        assert!(diff < last_diff, "Difference to target latency must converge monotonically");
        last_diff = diff;
    }

    assert!(last_diff < 500.0, "Weight should have converged close to the target of 5000.0");
}

#[test]
fn test_calibration_plan_selection_adaptation() {
    let optimizer = PlanOptimizer;
    
    // Heuristics 1: Default baseline weights
    let heuristics1 = CostHeuristics::default();
    
    // Heuristics 2: Heavily scaled vector search weight
    let heuristics2 = CostHeuristics {
        metadata: HeuristicMetadata { version: 2 },
        weights: HeuristicWeights {
            vector_weight: 500.0, // High cost
            keyword_weight: 2.0,
            expansion_weight: 5.0,
            fusion_weight: 1.0,
            ranking_weight: 0.5,
        },
    };

    let logical_plan = LogicalRetrievalPlan {
        steps: vec![
            LogicalStep::VectorRetrieve { query: "Rust".to_string() },
            LogicalStep::KeywordRetrieve { query: "Rust".to_string() },
        ],
    };

    let physical_plan1 = optimizer.optimize(logical_plan.clone(), &heuristics1);
    let physical_plan2 = optimizer.optimize(logical_plan, &heuristics2);

    assert_eq!(physical_plan1.heuristics_version, 1);
    assert_eq!(physical_plan2.heuristics_version, 2);

    // Assert cost calculation is affected by changed weights
    assert!(physical_plan2.cost.vector_cost > physical_plan1.cost.vector_cost);
    assert_eq!(physical_plan2.cost.keyword_cost, physical_plan1.cost.keyword_cost);
}
