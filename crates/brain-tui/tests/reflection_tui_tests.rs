use brain_integrations::dto::v1::{ReflectionFindingDto, ReflectionReport, ReflectionStatusReport, SkippedFindingDto};
use brain_tui::ui::theme::dark_theme;
use brain_tui::ui::widgets::reflection_panel::{draw, ReflectionPanelState};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

#[test]
fn test_reflection_panel_rendering() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let state = ReflectionPanelState {
        status: Some(ReflectionStatusReport {
            background_enabled: true,
            interval_secs: 60,
            min_events_trigger: 5,
            max_nodes_per_cycle: 1000,
            cycle_time_budget_ms: 5000,
            reflections_executed: 12,
            reflection_findings_count: 4,
            reflection_commands_executed: 3,
            reflection_commands_skipped: 1,
            last_reflection_duration_ms: Some(145),
        }),
        last_report: Some(ReflectionReport {
            execution_id: "test-exec-1234".to_string(),
            timestamp_ms: 1700000000000,
            duration_ms: 145,
            findings_processed: 4,
            commands_executed: 3,
            findings: vec![ReflectionFindingDto {
                kind: "duplicate".to_string(),
                confidence: 0.95,
                target_ids: vec!["node_1".to_string(), "node_2".to_string()],
                details: "High cosine similarity".to_string(),
            }],
            recommendations: vec![],
            executed_commands: vec!["Merged Node node_1 into node_2".to_string()],
            skipped_findings: vec![SkippedFindingDto {
                finding_kind: "link_suggestion".to_string(),
                confidence: 0.45,
                reasoning: "Below confidence threshold 0.70".to_string(),
            }],
            details: vec![],
        }),
        active_findings: vec![ReflectionFindingDto {
            kind: "duplicate".to_string(),
            confidence: 0.95,
            target_ids: vec!["node_1".to_string(), "node_2".to_string()],
            details: "High cosine similarity".to_string(),
        }],
        selected_finding_index: 0,
    };

    let theme = dark_theme();

    terminal
        .draw(|f| {
            let area = Rect::new(0, 0, 100, 30);
            draw(f, area, &state, theme, true);
        })
        .unwrap();

    let buffer = terminal.backend().buffer();

    // Verify key titles and telemetry elements rendered in the buffer
    let buffer_str = format!("{:?}", buffer);
    assert!(buffer_str.contains("Reflection Subsystem Inspector"));
    assert!(buffer_str.contains("Scheduler Telemetry"));
    assert!(buffer_str.contains("Active Findings (1)"));
    assert!(buffer_str.contains("Executed Commands & Decision Log"));
}
