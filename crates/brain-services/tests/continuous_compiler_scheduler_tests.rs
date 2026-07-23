use brain_services::compiler::{
    CoalescingDirtyBuffer, CompilationMode, CompileDecision, CompilerScheduler,
    CompilerSchedulerConfig, CompilerSchedulingPolicy, EntityIR, EntityId, FactId,
    KnowledgeCompiler, KnowledgeIR, ProvenanceIR, SchedulerState,
};

fn sample_prov() -> ProvenanceIR {
    ProvenanceIR {
        source_origin: "test_continuous".to_string(),
        evidence_ids: vec!["ev_001".to_string()],
        confidence: 0.90,
        timestamp_ms: 1700000000000,
    }
}

#[test]
fn test_compiler_scheduling_policy_evaluation() {
    let config = CompilerSchedulerConfig {
        background_enabled: true,
        interval_secs: 5,
        min_dirty_events_trigger: 3,
        max_batch_size: 1000,
        cycle_time_budget_ms: 10000,
    };
    let policy = CompilerSchedulingPolicy::new(config);

    // 0 pending events -> Wait
    assert_eq!(
        policy.evaluate(0, 1000, false),
        CompileDecision::Wait { pending_count: 0 }
    );

    // 2 pending events (< min_dirty_events_trigger) and recent compile -> Wait
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    assert_eq!(
        policy.evaluate(2, now_ms, false),
        CompileDecision::Wait { pending_count: 2 }
    );

    // 3 pending events (>= min_dirty_events_trigger) -> CompileNow
    assert_eq!(
        policy.evaluate(3, now_ms, false),
        CompileDecision::CompileNow {
            mode: CompilationMode::Incremental
        }
    );

    // Graph version mismatch -> ForceFull
    assert_eq!(policy.evaluate(1, now_ms, true), CompileDecision::ForceFull);
}

#[test]
fn test_coalescing_dirty_buffer_aggregation() {
    let buffer = CoalescingDirtyBuffer::new(1);
    assert_eq!(buffer.pending_count(), 0);

    buffer.mark_entity_dirty(EntityId("entity_a".to_string()));
    buffer.mark_entity_dirty(EntityId("entity_b".to_string()));
    buffer.mark_fact_dirty(FactId("fact_1".to_string()));
    // Duplicate insertion ignored
    buffer.mark_entity_dirty(EntityId("entity_a".to_string()));

    assert_eq!(buffer.pending_count(), 3);

    let dirty_set = buffer.drain(1);
    assert_eq!(buffer.pending_count(), 0);
    assert!(dirty_set.is_entity_dirty(&EntityId("entity_a".to_string())));
    assert!(dirty_set.is_entity_dirty(&EntityId("entity_b".to_string())));
    assert!(dirty_set.is_fact_dirty(&FactId("fact_1".to_string())));
}

#[test]
fn test_compiler_scheduler_step_produces_compilation_result() {
    let config = CompilerSchedulerConfig {
        background_enabled: true,
        interval_secs: 5,
        min_dirty_events_trigger: 1,
        max_batch_size: 1000,
        cycle_time_budget_ms: 10000,
    };
    let scheduler = CompilerScheduler::new(config);
    let compiler = KnowledgeCompiler::new();
    let buffer = CoalescingDirtyBuffer::new(1);

    let mut ir = KnowledgeIR::new();
    let entity = EntityIR::new(
        EntityId("entity_c".to_string()),
        "Concept Charlie",
        "concept",
        0.92,
        sample_prov(),
    );
    ir.insert_entity(entity);

    // Mark dirty to satisfy trigger
    buffer.mark_entity_dirty(EntityId("entity_c".to_string()));

    assert_eq!(scheduler.state(), SchedulerState::Stopped);

    // Run step
    let result = scheduler.run_step(&compiler, &buffer, &mut ir);

    assert!(result.is_some());
    let res = result.unwrap();
    assert_eq!(res.compiled_entities_count, 1);
    assert_eq!(res.report.passes_executed, 18);
    assert_eq!(res.graph_version, 1);

    // Verify buffer was drained
    assert_eq!(buffer.pending_count(), 0);
}
