use brain_integrations::dto::v1::{
    AutomationActionKind, AutomationExecutionLogDto, AutomationQueueItemDto, AutomationQueueStatus,
    AutomationRuleDto, AutomationTriggerKind,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::knowledge_automation::{
    draw_knowledge_automation_screen, KnowledgeAutomationIntent, KnowledgeAutomationNavigator,
    KnowledgeAutomationPanelFocus, KnowledgeAutomationState,
};
use brain_tui::ui::widgets::screen_state::ScreenState;
use brain_tui::ui::widgets::view_models::KnowledgeAutomationViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn sample_rules() -> Vec<AutomationRuleDto> {
    vec![AutomationRuleDto {
        rule_id: "rule_nightly_merge".to_string(),
        name: "Nightly Duplicate Entity Merge".to_string(),
        trigger_kind: AutomationTriggerKind::CronSchedule,
        action_kind: AutomationActionKind::AutoSimulateAndExecute,
        target_policy_id: "policy_merge_duplicates".to_string(),
        cron_expr: Some("0 2 * * *".to_string()),
        is_active: true,
        last_run_ms: None,
    }]
}

#[test]
fn test_knowledge_automation_screen_state_invariants() {
    let mut state = KnowledgeAutomationState::new();
    assert_eq!(state.selected_index(), 0);

    state.selected_rule_index = 2;
    assert_eq!(state.selected_index(), 2);

    state.reset();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(
        state.focused_panel,
        KnowledgeAutomationPanelFocus::RulesList
    );
}

#[test]
fn test_knowledge_automation_rendering_and_widgets() {
    let theme = Theme::default();
    let backend = TestBackend::new(120, 35);
    let mut terminal = Terminal::new(backend).unwrap();

    let rules = sample_rules();
    let queue = vec![AutomationQueueItemDto {
        queue_id: "queue_001".to_string(),
        automation_execution_id: "exec_001".to_string(),
        rule_id: "rule_nightly_merge".to_string(),
        target_policy_id: "policy_merge_duplicates".to_string(),
        status: AutomationQueueStatus::Queued,
        retry_count: 0,
        scheduled_at_ms: 1700000000000,
        started_at_ms: None,
        completed_at_ms: None,
    }];

    let logs = vec![AutomationExecutionLogDto {
        log_id: "log_001".to_string(),
        automation_execution_id: "exec_001".to_string(),
        rule_id: "rule_nightly_merge".to_string(),
        plan_id: Some("plan_001".to_string()),
        graph_version: 11,
        outcome_summary: "Successfully executed".to_string(),
        executed_at_ms: 1700000001000,
    }];

    let mut state = KnowledgeAutomationState::new();
    let vm = KnowledgeAutomationViewModel::from_data(&rules, Some(0), &queue, &logs);

    terminal
        .draw(|f| {
            draw_knowledge_automation_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("AUTOMATION ORCHESTRATION RULES"));
    assert!(buffer_str.contains("Nightly Duplicate Entity Merge"));
    assert!(buffer_str.contains("SCHEDULED EXECUTION QUEUE"));
    assert!(buffer_str.contains("exec_001"));

    // Test intent transition to focus queue timeline
    KnowledgeAutomationNavigator::process_intent(
        &mut state,
        KnowledgeAutomationIntent::TriggerSelectedRule,
    );
    assert_eq!(
        state.focused_panel,
        KnowledgeAutomationPanelFocus::QueueTimeline
    );
}
