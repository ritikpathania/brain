use brain_application::application::{BrainApplication, ReflectionProposalCommand};
use brain_integrations::dto::v1::{ProposalResolutionOutcome, ReflectionProposalStatus};
use brain_services::brain_runtime::BrainRuntime;
use std::sync::Arc;

#[tokio::test]
async fn test_idempotent_duplicate_proposal_command() {
    let runtime = Arc::new(BrainRuntime::new(":memory:").unwrap());
    let app = BrainApplication::new(runtime);

    // Initial list populates proposals
    let proposals = app.list_reflection_proposals().await.unwrap();
    assert!(!proposals.is_empty());
    let prop_id = proposals[0].proposal_id.clone();

    // 1st Command: Accept proposal
    let report1 = app
        .resolve_reflection_proposal(ReflectionProposalCommand::Accept {
            proposal_id: prop_id.clone(),
        })
        .await
        .unwrap();

    assert_eq!(report1.outcome, ProposalResolutionOutcome::Applied);
    assert_eq!(report1.status, ReflectionProposalStatus::Accepted);
    let v1 = report1.graph_version;

    // 2nd Command (Duplicate Replay): Accept proposal again
    let report2 = app
        .resolve_reflection_proposal(ReflectionProposalCommand::Accept {
            proposal_id: prop_id.clone(),
        })
        .await
        .unwrap();

    assert_eq!(report2.outcome, ProposalResolutionOutcome::AlreadyResolved);
    assert_eq!(report2.status, ReflectionProposalStatus::Accepted);
    assert_eq!(report2.graph_version, v1); // Version unchanged on idempotent replay
}

#[tokio::test]
async fn test_concurrent_proposal_resolution() {
    let runtime = Arc::new(BrainRuntime::new(":memory:").unwrap());
    let app = BrainApplication::new(runtime);

    let proposals = app.list_reflection_proposals().await.unwrap();
    let prop_id = proposals[0].proposal_id.clone();

    // First command resolves proposal
    let rep1 = app
        .resolve_reflection_proposal(ReflectionProposalCommand::Accept {
            proposal_id: prop_id.clone(),
        })
        .await
        .unwrap();
    assert_eq!(rep1.outcome, ProposalResolutionOutcome::Applied);

    // Second opposing command receives AlreadyResolved
    let rep2 = app
        .resolve_reflection_proposal(ReflectionProposalCommand::Reject {
            proposal_id: prop_id,
            reason: Some("Too late".to_string()),
        })
        .await
        .unwrap();
    assert_eq!(rep2.outcome, ProposalResolutionOutcome::AlreadyResolved);
    assert_eq!(rep2.status, ReflectionProposalStatus::Accepted);
}

#[tokio::test]
async fn test_projection_refresh_cycle() {
    let runtime = Arc::new(BrainRuntime::new(":memory:").unwrap());
    let app = BrainApplication::new(runtime);

    let proposals_before = app.list_reflection_proposals().await.unwrap();
    let prop_id = proposals_before[0].proposal_id.clone();

    let report = app
        .resolve_reflection_proposal(ReflectionProposalCommand::Accept {
            proposal_id: prop_id.clone(),
        })
        .await
        .unwrap();

    assert!(report.new_explanation_available);
    assert_eq!(report.affected_projection_count, 3);

    let proposals_after = app.list_reflection_proposals().await.unwrap();
    let updated_prop = proposals_after
        .iter()
        .find(|p| p.proposal_id == prop_id)
        .unwrap();
    assert_eq!(updated_prop.status, ReflectionProposalStatus::Accepted);
}
