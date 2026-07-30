use brain_domain::query::*;
use brain_services::query::physical_planner::*;

#[test]
fn test_physical_planner_creates_physical_plan() {
    let logical = LogicalPlan::Scan {
        target: ScanTarget::ActiveFacts,
    };

    let physical = PhysicalPlanner::plan(&logical).unwrap();
    assert_eq!(physical.root_name(), "PhysicalScan");
}
