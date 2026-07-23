use brain_integrations::dto::v1::{
    ExplanationReport, ExplanationStage, ExplanationStatus, ExplanationStepDto,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::explainability::{
    draw_explainability_screen, ExplainabilityIntent, ExplainabilityState, ExplanationNavigator,
};
use brain_tui::ui::widgets::view_models::ExplanationViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::collections::BTreeMap;

fn sample_explanation_report() -> ExplanationReport {
    let mut meta1 = BTreeMap::new();
    meta1.insert("source".to_string(), "/code/user.rs#L42".to_string());
    meta1.insert("correlation_id".to_string(), "corr_9981".to_string());

    let mut meta2 = BTreeMap::new();
    meta2.insert(
        "compiler_pass".to_string(),
        "CanonicalEntityResolutionPass".to_string(),
    );

    let steps = vec![
        ExplanationStepDto {
            step_id: "step_obs_001".to_string(),
            step_sequence: 1,
            parent_step_id: None,
            stage: ExplanationStage::Observation,
            status: ExplanationStatus::Success,
            title: "Observation Ingestion".to_string(),
            description: "Ingested raw knowledge snippet from file system".to_string(),
            timestamp_ms: 1700000000000,
            metadata: meta1,
        },
        ExplanationStepDto {
            step_id: "step_comp_002".to_string(),
            step_sequence: 2,
            parent_step_id: Some("step_obs_001".to_string()),
            stage: ExplanationStage::Compiler,
            status: ExplanationStatus::Success,
            title: "Compiler Normalization".to_string(),
            description: "CanonicalEntityResolutionPass established concept identity".to_string(),
            timestamp_ms: 1700000002000,
            metadata: meta2,
        },
        ExplanationStepDto {
            step_id: "step_refl_003".to_string(),
            step_sequence: 3,
            parent_step_id: Some("step_comp_002".to_string()),
            stage: ExplanationStage::Reflection,
            status: ExplanationStatus::Warning,
            title: "Reflection Finding Cycle".to_string(),
            description: "Reflection engine detected relationship candidate with confidence 0.88"
                .to_string(),
            timestamp_ms: 1700000005000,
            metadata: BTreeMap::new(),
        },
    ];

    ExplanationReport {
        concept_id: "node_user_001".to_string(),
        concept_label: "User".to_string(),
        node_type: "Person".to_string(),
        created_at_ms: 1700000000000,
        steps,
    }
}

#[test]
#[allow(clippy::useless_vec)]
fn test_deterministic_explanation_ordering_and_tie_breaker() {
    let mut steps = vec![
        ExplanationStepDto {
            step_id: "step_B".to_string(),
            step_sequence: 2,
            parent_step_id: Some("step_A".to_string()),
            stage: ExplanationStage::Compiler,
            status: ExplanationStatus::Success,
            title: "Pass B".to_string(),
            description: "Pass B description".to_string(),
            timestamp_ms: 1000, // Identical timestamp
            metadata: BTreeMap::new(),
        },
        ExplanationStepDto {
            step_id: "step_A".to_string(),
            step_sequence: 1,
            parent_step_id: None,
            stage: ExplanationStage::Observation,
            status: ExplanationStatus::Success,
            title: "Pass A".to_string(),
            description: "Pass A description".to_string(),
            timestamp_ms: 1000, // Identical timestamp
            metadata: BTreeMap::new(),
        },
    ];

    // Tie-breaker sorting rule: timestamp_ms then step_sequence
    steps.sort_by(|a, b| {
        a.timestamp_ms
            .cmp(&b.timestamp_ms)
            .then_with(|| a.step_sequence.cmp(&b.step_sequence))
    });

    assert_eq!(steps[0].step_id, "step_A");
    assert_eq!(steps[1].step_id, "step_B");
}

#[test]
fn test_partial_explanation_handling_without_reflection() {
    let report = ExplanationReport {
        concept_id: "node_test".to_string(),
        concept_label: "TestConcept".to_string(),
        node_type: "Topic".to_string(),
        created_at_ms: 1000,
        steps: vec![
            ExplanationStepDto {
                step_id: "step_1".to_string(),
                step_sequence: 1,
                parent_step_id: None,
                stage: ExplanationStage::Observation,
                status: ExplanationStatus::Success,
                title: "Obs".to_string(),
                description: "Obs desc".to_string(),
                timestamp_ms: 1000,
                metadata: BTreeMap::new(),
            },
            ExplanationStepDto {
                step_id: "step_2".to_string(),
                step_sequence: 2,
                parent_step_id: Some("step_1".to_string()),
                stage: ExplanationStage::Compiler,
                status: ExplanationStatus::Success,
                title: "Comp".to_string(),
                description: "Comp desc".to_string(),
                timestamp_ms: 2000,
                metadata: BTreeMap::new(),
            },
        ],
    };

    let vm = ExplanationViewModel::from_report(Some(&report), Some(0));
    assert_eq!(vm.timeline.items.len(), 2);
    assert_eq!(vm.summary.unwrap().total_steps_text, "2");
}

#[test]
fn test_missing_historical_records_placeholders() {
    let theme = Theme::default();
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    let state = ExplainabilityState::new();
    let vm = ExplanationViewModel::from_report(None, None);

    terminal
        .draw(|f| {
            draw_explainability_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("No explanation report loaded"));
    assert!(buffer_str.contains("No causal execution steps available"));
}

#[test]
fn test_explanation_report_stability() {
    let report = sample_explanation_report();
    let vm1 = ExplanationViewModel::from_report(Some(&report), Some(0));
    let vm2 = ExplanationViewModel::from_report(Some(&report), Some(0));

    assert_eq!(vm1, vm2);
}

#[test]
fn test_timeline_scrolling_and_selection() {
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut report = sample_explanation_report();
    for i in 4..=15 {
        report.steps.push(ExplanationStepDto {
            step_id: format!("step_{}", i),
            step_sequence: i as u64,
            parent_step_id: Some(format!("step_{}", i - 1)),
            stage: ExplanationStage::Projection,
            status: ExplanationStatus::Info,
            title: format!("Projection Batch Update {}", i),
            description: "Projection sync".to_string(),
            timestamp_ms: 1700000000000 + (i as u64 * 1000),
            metadata: BTreeMap::new(),
        });
    }

    let mut state = ExplainabilityState::new();
    state.selected_step_index = 2; // Selected index 2 (Reflection warning)

    let vm = ExplanationViewModel::from_report(Some(&report), Some(state.selected_step_index));

    terminal
        .draw(|f| {
            draw_explainability_screen(f, f.size(), &vm, &state, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("CHRONOLOGICAL CAUSAL TIMELINE"));
    assert!(buffer_str.contains("⚠"));
    assert!(buffer_str.contains("Reflection Finding Cycle"));

    // Test Navigator intent transition
    ExplanationNavigator::process_intent(&mut state, ExplainabilityIntent::SelectStepNext);
    assert_eq!(state.selected_step_index, 3);
}

#[test]
fn test_broken_causal_chain_parent_unavailable() {
    let report = ExplanationReport {
        concept_id: "node_broken".to_string(),
        concept_label: "BrokenChainNode".to_string(),
        node_type: "System".to_string(),
        created_at_ms: 1000,
        steps: vec![ExplanationStepDto {
            step_id: "step_2".to_string(),
            step_sequence: 2,
            // Parent ID points to non-existent / compacted step
            parent_step_id: Some("step_compacted_000".to_string()),
            stage: ExplanationStage::Compiler,
            status: ExplanationStatus::Success,
            title: "Normalized Fact".to_string(),
            description: "Compiled fact".to_string(),
            timestamp_ms: 2000,
            metadata: BTreeMap::new(),
        }],
    };

    let vm = ExplanationViewModel::from_report(Some(&report), Some(0));
    assert!(vm.detail_pane.is_some());
    let detail = vm.detail_pane.unwrap();
    assert!(detail.parent_step_id_text.contains("step_compacted_000"));
    assert!(detail.parent_step_id_text.contains("unavailable"));
}
