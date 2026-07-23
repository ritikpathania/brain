use brain_domain::SessionId;
use brain_services::compiler::{
    CompilerContext, CompilerDependencyGraph, DirtySet, EntityIR, EntityId, FactIR, FactId,
    KnowledgeCompiler, KnowledgeIR, ProvenanceIR, RelationIR,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

fn generate_synthetic_ir(num_entities: usize, num_facts: usize) -> KnowledgeIR {
    let mut ir = KnowledgeIR::new();

    let prov = ProvenanceIR {
        source_origin: "bench_harness".to_string(),
        evidence_ids: vec!["ev_bench".to_string()],
        confidence: 0.88,
        timestamp_ms: 1700000000000,
    };

    for i in 0..num_entities {
        let entity_id = EntityId(format!("entity_{}", i));
        let mut entity = EntityIR::new(
            entity_id.clone(),
            format!("  Canonical Concept {}  ", i),
            "concept",
            0.85,
            prov.clone(),
        );
        entity.aliases = vec![format!("ConceptAlias_{}", i)];
        ir.insert_entity(entity);
    }

    for i in 0..num_facts {
        let subject_idx = i % num_entities;
        let fact = FactIR::new(
            FactId(format!("fact_{}", i)),
            EntityId(format!("entity_{}", subject_idx)),
            "property",
            format!("Value_{}", i),
            0.90,
            prov.clone(),
        );
        ir.insert_fact(fact);
    }

    for i in 0..(num_entities / 2) {
        let source_id = EntityId(format!("entity_{}", i));
        let target_id = EntityId(format!("entity_{}", (i + 1) % num_entities));
        ir.add_relation(RelationIR {
            source_id,
            target_id,
            relation_kind: "connects_to".to_string(),
            weight: 0.75,
            provenance: prov.clone(),
            provenance_chain: vec![prov.clone()],
        });
    }

    ir
}

fn bench_full_compilation(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_full_execution");
    let compiler = KnowledgeCompiler::new();

    for size in [100, 500, 1000] {
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &s| {
            let context = CompilerContext {
                compilation_id: Uuid::new_v4(),
                session_id: SessionId::new(),
                graph_version: 1,
                dirty_set: None,
                min_confidence_threshold: 0.70,
                time_budget_ms: 30000,
                cancellation_token: CancellationToken::new(),
                config: brain_services::compiler::CompilerOptimizationConfig::default(),
            };

            b.iter(|| {
                let mut ir = generate_synthetic_ir(s, s);
                let (_compiled_ir, report) = compiler.compile(&context, &mut ir);
                criterion::black_box((report.passes_executed, report.entities_compiled));
            });
        });
    }
    group.finish();
}

fn bench_incremental_compilation_speedup(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_incremental_speedup");
    let compiler = KnowledgeCompiler::new();

    let context = CompilerContext {
        compilation_id: Uuid::new_v4(),
        session_id: SessionId::new(),
        graph_version: 1,
        dirty_set: None,
        min_confidence_threshold: 0.70,
        time_budget_ms: 30000,
        cancellation_token: CancellationToken::new(),
        config: brain_services::compiler::CompilerOptimizationConfig::default(),
    };

    group.bench_function("full_compilation_1000", |b| {
        b.iter(|| {
            let mut ir = generate_synthetic_ir(1000, 1000);
            let (_compiled_ir, report) = compiler.compile(&context, &mut ir);
            criterion::black_box(report);
        });
    });

    group.bench_function("incremental_compilation_10percent_dirty", |b| {
        b.iter(|| {
            let mut ir = generate_synthetic_ir(1000, 1000);
            let mut dirty_set = DirtySet::new(1);
            for i in 0..100 {
                dirty_set.mark_entity(EntityId(format!("entity_{}", i)));
            }
            let (_compiled_ir, report) = compiler.compile_incremental(&context, &mut ir, dirty_set);
            criterion::black_box(report);
        });
    });

    group.finish();
}

fn bench_dirty_set_expansion(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_dirty_set_expansion");
    let ir = generate_synthetic_ir(1000, 1000);
    let dep_graph = CompilerDependencyGraph::build_from_ir(&ir);

    group.bench_function("expand_dirty_set_100_nodes", |b| {
        b.iter(|| {
            let mut dirty_set = DirtySet::new(1);
            for i in 0..100 {
                dirty_set.mark_entity(EntityId(format!("entity_{}", i)));
            }
            let expanded = dep_graph.expand_dirty_set(&dirty_set);
            criterion::black_box(expanded);
        });
    });

    group.finish();
}

fn bench_telemetry_snapshot_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler_observability_overhead");
    let compiler = KnowledgeCompiler::new();

    group.bench_function("live_snapshot_single_read", |b| {
        b.iter(|| {
            let snap = compiler.runtime_state().live_snapshot();
            criterion::black_box(snap);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_full_compilation,
    bench_incremental_compilation_speedup,
    bench_dirty_set_expansion,
    bench_telemetry_snapshot_overhead
);
criterion_main!(benches);
