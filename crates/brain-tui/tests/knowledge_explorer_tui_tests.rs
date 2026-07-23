use brain_integrations::dto::v1::{
    ConceptDetailReport, ConceptSummaryDto, ProvenanceDetailDto, RelationDetailDto,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::knowledge_explorer::{
    draw_knowledge_explorer, ExplorerIntent, GraphNavigator, KnowledgeExplorerState,
};
use brain_tui::ui::widgets::view_models::KnowledgeExplorerViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::BTreeMap;

fn sample_concept_summaries() -> Vec<ConceptSummaryDto> {
    vec![
        ConceptSummaryDto {
            id: "node_user_001".to_string(),
            label: "User".to_string(),
            node_type: "Person".to_string(),
            relationships_count: 2,
        },
        ConceptSummaryDto {
            id: "node_brain_002".to_string(),
            label: "Brain Engine".to_string(),
            node_type: "System".to_string(),
            relationships_count: 1,
        },
        ConceptSummaryDto {
            id: "node_rust_003".to_string(),
            label: "Rust Language".to_string(),
            node_type: "Language".to_string(),
            relationships_count: 0,
        },
    ]
}

fn sample_concept_detail(id: &str, label: &str, node_type: &str) -> ConceptDetailReport {
    let mut props = BTreeMap::new();
    props.insert("sys_version".to_string(), "v1.0.0".to_string());
    props.insert("label".to_string(), label.to_string());
    props.insert("user_role".to_string(), "Architect".to_string());
    props.insert("description".to_string(), "Core user profile".to_string());

    let relations = vec![
        RelationDetailDto {
            target_id: "node_rust_003".to_string(),
            target_label: "Rust Language".to_string(),
            target_type: "Language".to_string(),
            relation: "knows".to_string(),
            direction: "outgoing".to_string(),
            weight: 0.88,
        },
        RelationDetailDto {
            target_id: "node_brain_002".to_string(),
            target_label: "Brain Engine".to_string(),
            target_type: "System".to_string(),
            relation: "works_on".to_string(),
            direction: "outgoing".to_string(),
            weight: 0.95,
        },
        RelationDetailDto {
            target_id: "node_org_999".to_string(),
            target_label: "Organization".to_string(),
            target_type: "Group".to_string(),
            relation: "member_of".to_string(),
            direction: "incoming".to_string(),
            weight: 1.00,
        },
    ];

    let mut extra = BTreeMap::new();
    extra.insert("confidence".to_string(), "0.99".to_string());

    let provenance = ProvenanceDetailDto {
        source: "Ingested".to_string(),
        compiler_pass: Some("CanonicalEntityResolutionPass".to_string()),
        location: "/src/user.rs#L42".to_string(),
        timestamp_ms: 1700000000000,
        extra_info: extra,
    };

    ConceptDetailReport {
        id: id.to_string(),
        label: label.to_string(),
        node_type: node_type.to_string(),
        properties: props,
        relations,
        provenance,
    }
}

#[test]
fn test_explorer_layout_and_widget_rendering() {
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let summaries = sample_concept_summaries();
    let detail = sample_concept_detail("node_user_001", "User", "Person");
    let state = KnowledgeExplorerState::new();

    let vm = KnowledgeExplorerViewModel::from_report(&summaries, Some(&detail), Some(0), Some(0));

    terminal
        .draw(|f| {
            draw_knowledge_explorer(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("CONCEPTS CATALOG"));
    assert!(buffer_str.contains("CONCEPT DETAILS"));
    assert!(buffer_str.contains("RELATIONSHIPS & ADJACENCY"));
    assert!(buffer_str.contains("PROVENANCE & ORIGIN HISTORY"));
    assert!(buffer_str.contains("User"));
    assert!(buffer_str.contains("Brain Engine"));
}

#[test]
fn test_deterministic_relation_and_property_ordering() {
    let summaries = sample_concept_summaries();
    let detail = sample_concept_detail("node_user_001", "User", "Person");

    let vm = KnowledgeExplorerViewModel::from_report(&summaries, Some(&detail), Some(0), Some(0));

    // 1. Verify Relations Ordering: Outgoing -> Incoming -> Relation -> Target label
    assert_eq!(vm.relations.items.len(), 3);
    assert_eq!(vm.relations.items[0].direction, "OUTGOING");
    assert_eq!(vm.relations.items[0].relation, "knows");
    assert_eq!(vm.relations.items[1].direction, "OUTGOING");
    assert_eq!(vm.relations.items[1].relation, "works_on");
    assert_eq!(vm.relations.items[2].direction, "INCOMING");
    assert_eq!(vm.relations.items[2].relation, "member_of");

    // 2. Verify Property Group Ordering: System -> Canonical -> User -> Metadata
    let groups: Vec<String> = vm
        .properties
        .items
        .iter()
        .map(|p| p.group.clone())
        .collect();
    assert_eq!(groups, vec!["System", "Canonical", "User", "Metadata"]);
}

#[test]
fn test_breadcrumb_stack_navigation() {
    let mut state = KnowledgeExplorerState::new();

    // Start at A
    GraphNavigator::process_intent(
        &mut state,
        ExplorerIntent::JumpToTarget {
            target_id: "node_A".to_string(),
        },
    );
    assert_eq!(state.selected_concept_id, Some("node_A".to_string()));
    assert!(state.history_stack.is_empty());

    // Jump to B
    GraphNavigator::process_intent(
        &mut state,
        ExplorerIntent::JumpToTarget {
            target_id: "node_B".to_string(),
        },
    );
    assert_eq!(state.selected_concept_id, Some("node_B".to_string()));
    assert_eq!(state.history_stack, vec!["node_A"]);

    // Jump to C
    GraphNavigator::process_intent(
        &mut state,
        ExplorerIntent::JumpToTarget {
            target_id: "node_C".to_string(),
        },
    );
    assert_eq!(state.selected_concept_id, Some("node_C".to_string()));
    assert_eq!(state.history_stack, vec!["node_A", "node_B"]);

    // Press Back ('b') -> returns to B
    GraphNavigator::process_intent(&mut state, ExplorerIntent::NavigateBack);
    assert_eq!(state.selected_concept_id, Some("node_B".to_string()));
    assert_eq!(state.history_stack, vec!["node_A"]);
    assert_eq!(state.forward_stack, vec!["node_C"]);

    // Press Forward ('Shift+B') -> returns to C
    GraphNavigator::process_intent(&mut state, ExplorerIntent::NavigateForward);
    assert_eq!(state.selected_concept_id, Some("node_C".to_string()));
    assert_eq!(state.history_stack, vec!["node_A", "node_B"]);
    assert!(state.forward_stack.is_empty());
}

#[test]
fn test_cyclic_graph_navigation() {
    let mut state = KnowledgeExplorerState::new();

    // Cycle A -> B -> C -> A
    let nodes = vec!["node_A", "node_B", "node_C", "node_A"];
    for node_id in nodes {
        GraphNavigator::process_intent(
            &mut state,
            ExplorerIntent::JumpToTarget {
                target_id: node_id.to_string(),
            },
        );
    }

    assert_eq!(state.selected_concept_id, Some("node_A".to_string()));
    assert_eq!(state.history_stack, vec!["node_A", "node_B", "node_C"]);

    // Back 3 times back to start
    GraphNavigator::process_intent(&mut state, ExplorerIntent::NavigateBack);
    assert_eq!(state.selected_concept_id, Some("node_C".to_string()));

    GraphNavigator::process_intent(&mut state, ExplorerIntent::NavigateBack);
    assert_eq!(state.selected_concept_id, Some("node_B".to_string()));

    GraphNavigator::process_intent(&mut state, ExplorerIntent::NavigateBack);
    assert_eq!(state.selected_concept_id, Some("node_A".to_string()));
}

#[test]
fn test_missing_target_node_handling() {
    let theme = Theme::default();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut state = KnowledgeExplorerState::new();
    state.missing_node_error = Some("Node missing_999 not found in graph store".to_string());

    let summaries = sample_concept_summaries();
    let vm = KnowledgeExplorerViewModel::from_report(&summaries, None, Some(0), None);

    terminal
        .draw(|f| {
            draw_knowledge_explorer(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("Target Concept Unavailable"));
    assert!(buffer_str.contains("missing_999 not found"));
}
