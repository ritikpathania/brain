use brain_integrations::dto::v1::{
    ReflectionActionType, ReflectionProposalDto, ReflectionProposalStatus,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::interactive_reflection::{
    draw_interactive_reflection_screen, InteractiveReflectionIntent,
    InteractiveReflectionPanelFocus, InteractiveReflectionState, ReflectionProposalNavigator,
};
use brain_tui::ui::widgets::screen_state::ScreenState;
use brain_tui::ui::widgets::view_models::InteractiveReflectionViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn sample_proposals() -> Vec<ReflectionProposalDto> {
    vec![
        ReflectionProposalDto {
            proposal_id: "prop_001".to_string(),
            finding_kind: "duplicate_entity_candidate".to_string(),
            source_concept_id: "node_user_001".to_string(),
            target_concept_id: Some("node_person_002".to_string()),
            confidence: 0.94,
            action_type: ReflectionActionType::MergeEntities,
            explanation_summary: "Duplicate concepts 'User' and 'Person' share 94% similarity"
                .to_string(),
            status: ReflectionProposalStatus::Pending,
            created_at_ms: 1700000000000,
            resolved_at_ms: None,
            resolved_graph_version: None,
        },
        ReflectionProposalDto {
            proposal_id: "prop_002".to_string(),
            finding_kind: "adjacency_strengthening".to_string(),
            source_concept_id: "node_brain".to_string(),
            target_concept_id: Some("node_sqlite".to_string()),
            confidence: 0.88,
            action_type: ReflectionActionType::StrengthenEdge,
            explanation_summary: "Co-occurrence suggests strengthening edge".to_string(),
            status: ReflectionProposalStatus::Pending,
            created_at_ms: 1700000002000,
            resolved_at_ms: None,
            resolved_graph_version: None,
        },
    ]
}

#[test]
fn test_screen_state_trait_invariants() {
    let mut state = InteractiveReflectionState::new();
    assert_eq!(state.selected_index(), 0);

    state.selected_proposal_index = 5;
    assert_eq!(state.selected_index(), 5);

    state.reset();
    assert_eq!(state.selected_index(), 0);
    assert_eq!(
        state.focused_panel,
        InteractiveReflectionPanelFocus::ProposalList
    );
}

#[test]
fn test_interactive_reflection_rendering_and_modal() {
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let proposals = sample_proposals();
    let mut state = InteractiveReflectionState::new();
    let vm = InteractiveReflectionViewModel::from_proposals(&proposals, Some(0), None);

    terminal
        .draw(|f| {
            draw_interactive_reflection_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("REFLECTION PROPOSALS"));
    assert!(buffer_str.contains("MERGE"));
    assert!(buffer_str.contains("prop_001"));

    // Test intent transition to open confirmation modal
    state.pending_confirmation = Some(("Accept".to_string(), "prop_001".to_string()));
    ReflectionProposalNavigator::process_intent(
        &mut state,
        InteractiveReflectionIntent::AcceptSelected,
    );
    assert_eq!(
        state.focused_panel,
        InteractiveReflectionPanelFocus::ConfirmationModal
    );

    terminal
        .draw(|f| {
            draw_interactive_reflection_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let modal_str = format!("{:?}", terminal.backend().buffer());
    assert!(modal_str.contains("CONFIRM PROPOSAL ACTION: ACCEPT"));
}
