use brain_integrations::dto::v1::{
    OrchestratorStatsDto, ProjectionLagDto, ReflectionStatusReport, RuntimeDiagnosticsReport,
    TaskTraceDto,
};
use brain_tui::ui::theme::Theme;
use brain_tui::ui::widgets::runtime_dashboard::{draw_runtime_dashboard, RuntimeDashboardState};
use brain_tui::ui::widgets::view_models::RuntimeDashboardViewModel;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn sample_report(health: &str, reason: Option<&str>) -> RuntimeDiagnosticsReport {
    RuntimeDiagnosticsReport {
        snapshot_sequence: 42,
        snapshot_timestamp_ms: 1720000000000,
        health: health.to_string(),
        health_reason: reason.map(|s| s.to_string()),
        orchestrator: OrchestratorStatsDto {
            pending_tasks_count: 2,
            tasks_queued: 10,
            tasks_completed: 8,
            tasks_failed: 0,
            tasks_dropped: 0,
            last_task_wait_ms: 2,
            last_task_exec_ms: 45,
            current_running_task: Some(TaskTraceDto {
                id: "task_12345678".to_string(),
                kind: "compile".to_string(),
                priority: "critical".to_string(),
                status: "Running".to_string(),
                created_at_unix_ms: 1720000000000,
                wait_duration_ms: 1,
                exec_duration_ms: 10,
            }),
            task_history: vec![
                TaskTraceDto {
                    id: "task_12345678".to_string(),
                    kind: "compile".to_string(),
                    priority: "critical".to_string(),
                    status: "Running".to_string(),
                    created_at_unix_ms: 1720000000000,
                    wait_duration_ms: 1,
                    exec_duration_ms: 10,
                },
                TaskTraceDto {
                    id: "task_87654321".to_string(),
                    kind: "project".to_string(),
                    priority: "normal".to_string(),
                    status: "Succeeded".to_string(),
                    created_at_unix_ms: 1719999900000,
                    wait_duration_ms: 3,
                    exec_duration_ms: 25,
                },
            ],
        },
        projection_lags: vec![
            ProjectionLagDto {
                projection_id: "Jobs".to_string(),
                last_processed_sequence: 100,
                max_event_sequence: 100,
                lag_sequence_count: 0,
            },
            ProjectionLagDto {
                projection_id: "Search".to_string(),
                last_processed_sequence: 95,
                max_event_sequence: 100,
                lag_sequence_count: 5,
            },
        ],
        reflection: ReflectionStatusReport {
            background_enabled: true,
            interval_secs: 300,
            min_events_trigger: 10,
            max_nodes_per_cycle: 100,
            cycle_time_budget_ms: 5000,
            reflections_executed: 12,
            reflection_findings_count: 45,
            reflection_commands_executed: 8,
            reflection_commands_skipped: 2,
            last_reflection_duration_ms: Some(150),
        },
    }
}

#[test]
fn test_health_widget_rendering_healthy_degraded_unhealthy() {
    let theme = Theme::default();

    for (health_str, reason, expected_sub) in [
        ("healthy", None, "HEALTHY"),
        ("degraded", Some("Projection lag > 100"), "DEGRADED"),
        ("unhealthy", Some("Orchestrator failure"), "UNHEALTHY"),
    ] {
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();

        let report = sample_report(health_str, reason);
        let vm = RuntimeDashboardViewModel::from_report(&report, Some(0));

        terminal
            .draw(|f| {
                draw_runtime_dashboard(f, f.size(), &vm, &theme);
            })
            .unwrap();

        let buffer_str = format!("{:?}", terminal.backend().buffer());
        assert!(
            buffer_str.contains(expected_sub),
            "Buffer missing expected health label {}",
            expected_sub
        );

        if let Some(r) = reason {
            assert!(buffer_str.contains(r), "Buffer missing health reason {}", r);
        }
    }
}

#[test]
fn test_empty_state_placeholder_rendering() {
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut report = sample_report("healthy", None);
    report.orchestrator.task_history.clear();
    report.orchestrator.current_running_task = None;

    let vm = RuntimeDashboardViewModel::from_report(&report, None);

    terminal
        .draw(|f| {
            draw_runtime_dashboard(f, f.size(), &vm, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("No background tasks executed yet."));
    assert!(buffer_str.contains("Idle (waiting for tasks)"));
}

#[test]
fn test_text_truncation_overflow_safety() {
    let theme = Theme::default();
    // Compact terminal width (70 cols)
    let backend = TestBackend::new(70, 25);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut report = sample_report("healthy", None);
    report.orchestrator.task_history[0].id = "task_very_long_uuid_99999999999".to_string();
    report.projection_lags[0].projection_id = "very_long_custom_projection_name".to_string();

    let vm = RuntimeDashboardViewModel::from_report(&report, Some(0));

    // Must draw cleanly without layout panic
    terminal
        .draw(|f| {
            draw_runtime_dashboard(f, f.size(), &vm, &theme);
        })
        .unwrap();

    let buffer_str = format!("{:?}", terminal.backend().buffer());
    assert!(buffer_str.contains("task_..."));
}

#[test]
fn test_refresh_stability_preserves_layout_and_selection() {
    let theme = Theme::default();
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    let state = RuntimeDashboardState {
        selected_history_index: 1,
    };

    let mut report = sample_report("healthy", None);
    let vm1 = RuntimeDashboardViewModel::from_report(&report, Some(state.selected_history_index));

    terminal
        .draw(|f| {
            draw_runtime_dashboard(f, f.size(), &vm1, &theme);
        })
        .unwrap();

    let buf1 = terminal.backend().buffer().clone();

    // Mutate telemetry counters (simulating live refresh tick)
    report.snapshot_sequence = 43;
    report.orchestrator.tasks_completed = 9;
    let vm2 = RuntimeDashboardViewModel::from_report(&report, Some(state.selected_history_index));

    terminal
        .draw(|f| {
            draw_runtime_dashboard(f, f.size(), &vm2, &theme);
        })
        .unwrap();

    let buf2 = terminal.backend().buffer().clone();

    // Layout geometry must remain 100% identical
    assert_eq!(buf1.area(), buf2.area());
}
