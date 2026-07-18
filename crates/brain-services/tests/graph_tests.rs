use brain_core::errors::BrainError;
use brain_services::agent::engine::ExecutionStage;
use brain_services::agent::graph::{RetryPolicy, WorkflowGraphBuilder, WorkflowNode};
use brain_services::agent::{ExecutionContext, ExecutionState, StageIdentifier, StageOutcome};

struct StubStage(StageIdentifier, bool);

impl ExecutionStage for StubStage {
    fn name(&self) -> &'static str {
        "stub"
    }

    fn id(&self) -> StageIdentifier {
        self.0
    }

    fn supports_retry(&self) -> bool {
        self.1
    }

    fn execute(
        &self,
        _ctx: &ExecutionContext,
        _state: &mut ExecutionState,
    ) -> Result<StageOutcome, BrainError> {
        Ok(StageOutcome::Continue)
    }
}

#[test]
fn test_valid_linear_graph() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Planning)
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: Some(StageIdentifier::Reasoning),
                policies: vec![],
            },
        )
        .node(
            StageIdentifier::Reasoning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
                next_stage: None,
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_ok());
}

#[test]
fn test_missing_start_node() {
    let g = WorkflowGraphBuilder::new()
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: None,
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_dangling_edge() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Planning)
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: Some(StageIdentifier::Reasoning),
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_unreachable_node() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Planning)
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: None,
                policies: vec![],
            },
        )
        .node(
            StageIdentifier::Reasoning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
                next_stage: None,
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_sequential_cycle() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Planning)
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: Some(StageIdentifier::Reasoning),
                policies: vec![],
            },
        )
        .node(
            StageIdentifier::Reasoning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Reasoning, false)),
                next_stage: Some(StageIdentifier::Planning),
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_reflection_lacks_retry_policy() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Reflection)
        .node(
            StageIdentifier::Reflection,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Reflection, true)),
                next_stage: None,
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_duplicate_policies() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Reflection)
        .node(
            StageIdentifier::Reflection,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Reflection, true)),
                next_stage: None,
                policies: vec![
                    Box::new(RetryPolicy { max_attempts: 3 }),
                    Box::new(RetryPolicy { max_attempts: 2 }),
                ],
            },
        )
        .build();
    assert!(g.is_err());
}

#[test]
fn test_no_terminal_path() {
    let g = WorkflowGraphBuilder::new()
        .start_node(StageIdentifier::Planning)
        .node(
            StageIdentifier::Planning,
            WorkflowNode {
                stage: Box::new(StubStage(StageIdentifier::Planning, false)),
                next_stage: Some(StageIdentifier::Planning),
                policies: vec![],
            },
        )
        .build();
    assert!(g.is_err());
}
