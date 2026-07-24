use brain_integrations::dto::v1::{
    EvolutionActionKind, EvolutionAuditRecordDto, EvolutionExecutionOutcome, EvolutionPlanDto,
    EvolutionPlanStatus, EvolutionPolicyDto, EvolutionSimulationReport, EvolutionStepDto,
    EvolutionTriggerKind,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::knowledge_evolution::{
    draw_knowledge_evolution_screen, KnowledgeEvolutionIntent, KnowledgeEvolutionNavigator,
    KnowledgeEvolutionPanelFocus, KnowledgeEvolutionState,
};
use brain_tui::ui::widgets::screen_state::ScreenState;
use brain_tui::ui::widgets::view_models::KnowledgeEvolutionViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn sample_policies() -> Vec<EvolutionPolicyDto> {
    vec![EvolutionPolicyDto {
        policy_id: "policy_merge_duplicates".to_string(),
        priority: 10,
        name: "Merge Duplicate Entities Policy".to_string(),
        description: "Automatically merge duplicate entities.".to_string(),
        trigger_kind: EvolutionTriggerKind::HighSimilarityDuplicate,
        action_kind: EvolutionActionKind::MergeEntities,
        auto_apply: false,
        created_at_ms: 1700000000000,
    }]
}

#[test]
fn test_knowledge_evolution_screen_state_invariants() {
    let mut state = KnowledgeEvolutionState::new();
    assert_eq!(state.selected_index(), 0);

    state.selected_policy_index = 3;
    assert_eq!(state.selected_index(), 3);

    state.reset();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(
        state.focused_panel,
        KnowledgeEvolutionPanelFocus::PoliciesList
    );
}

#[test]
fn test_knowledge_evolution_rendering_and_widgets() {
    let theme = Theme::default();
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    let policies = sample_policies();
    let plan = EvolutionPlanDto {
        plan_id: "plan_merge_duplicates".to_string(),
        target_graph_version: 10,
        policy_id: "policy_merge_duplicates".to_string(),
        status: EvolutionPlanStatus::Draft,
        steps: vec![EvolutionStepDto {
            step_id: "step_001".to_string(),
            sequence: 1,
            action_kind: EvolutionActionKind::MergeEntities,
            target_id: "node_user_001".to_string(),
            secondary_id: Some("node_person_002".to_string()),
            description: "Merge duplicate node".to_string(),
        }],
        created_at_ms: 1700000000000,
    };

    let sim_report = EvolutionSimulationReport {
        plan_id: "plan_merge_duplicates".to_string(),
        simulated_at_ms: 1700000001000,
        entities_affected_count: 1,
        facts_retired_count: 0,
        edges_strengthened_count: 0,
        confidence_delta: 0.12,
        risk_score: 0.05,
        risk_level: "LOW".to_string(),
        affected_concept_ids: vec!["node_user_001".to_string()],
    };

    let audit_records = vec![EvolutionAuditRecordDto {
        audit_id: "audit_001".to_string(),
        graph_version: 11,
        plan_id: "plan_merge_duplicates".to_string(),
        policy_name: "Merge Policy".to_string(),
        executed_at_ms: 1700000002000,
        outcome: EvolutionExecutionOutcome::Applied,
        steps_applied_count: 1,
        summary: "Successfully executed plan".to_string(),
    }];

    let mut state = KnowledgeEvolutionState::new();
    let vm = KnowledgeEvolutionViewModel::from_data(
        &policies,
        Some(0),
        Some(&plan),
        Some(&sim_report),
        &audit_records,
    );

    terminal
        .draw(|f| {
            draw_knowledge_evolution_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("GOVERNANCE EVOLUTION POLICIES"));
    assert!(buffer_str.contains("Merge Duplicate Entities Policy"));
    assert!(buffer_str.contains("plan_merge_duplicates"));
    assert!(buffer_str.contains("LOW RISK"));

    // Test intent transition to focus plan timeline
    KnowledgeEvolutionNavigator::process_intent(
        &mut state,
        KnowledgeEvolutionIntent::GeneratePlanForSelected,
    );
    assert_eq!(
        state.focused_panel,
        KnowledgeEvolutionPanelFocus::PlanTimeline
    );
}
