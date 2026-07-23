//! Pure domain telemetry models for Knowledge Compiler execution.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Mode of compilation execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompilationMode {
    /// Full re-compilation over the entire Knowledge IR.
    Full,
    /// Incremental compilation over a dirty subset of Knowledge IR.
    Incremental,
}

impl std::fmt::Display for CompilationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilationMode::Full => write!(f, "full"),
            CompilationMode::Incremental => write!(f, "incremental"),
        }
    }
}

/// Static identifier for compiler passes to ensure stable telemetry keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PassId {
    /// Observation normalization pass
    ObservationNormalization,
    /// Alias resolution pass
    AliasResolution,
    /// Entity merge pass
    EntityMerge,
    /// Canonical entity resolution pass
    CanonicalEntityResolution,
    /// Confidence aggregation pass
    ConfidenceAggregation,
    /// Provenance merge pass
    ProvenanceMerge,
    /// Fact deduplication pass
    FactDeduplication,
    /// Temporal fact resolution pass
    TemporalFactResolution,
    /// Canonical fact selection pass
    CanonicalFactSelection,
    /// Relation normalization pass
    RelationNormalization,
    /// Compiler contradiction pass
    CompilerContradiction,
    /// Graph validation pass
    Validation,
    /// Custom pass extension
    Custom(&'static str),
}

impl PassId {
    /// Returns static string representation of the pass ID.
    pub fn as_str(&self) -> &'static str {
        match self {
            PassId::ObservationNormalization => "observation_normalization",
            PassId::AliasResolution => "alias_resolution",
            PassId::EntityMerge => "entity_merge",
            PassId::CanonicalEntityResolution => "canonical_entity_resolution",
            PassId::ConfidenceAggregation => "confidence_aggregation",
            PassId::ProvenanceMerge => "provenance_merge",
            PassId::FactDeduplication => "fact_deduplication",
            PassId::TemporalFactResolution => "temporal_fact_resolution",
            PassId::CanonicalFactSelection => "canonical_fact_selection",
            PassId::RelationNormalization => "relation_normalization",
            PassId::CompilerContradiction => "compiler_contradiction",
            PassId::Validation => "validation",
            PassId::Custom(s) => s,
        }
    }
}

/// Emitted execution record captured during a single compiler pass execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassExecutionRecord {
    /// Stable pass identifier.
    pub pass_id: PassId,
    /// Execution duration in nanoseconds.
    pub duration_ns: u64,
    /// Diagnostics count emitted during pass.
    pub diagnostics_emitted: usize,
}

/// Aggregated performance metrics for a single compiler pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassMetrics {
    /// Pass static identifier string.
    pub pass_name: String,
    /// Total times this pass has been executed.
    pub executions: u64,
    /// Total duration spent in this pass in nanoseconds.
    pub total_duration_ns: u64,
}

impl PassMetrics {
    /// Computes average duration per execution in milliseconds.
    pub fn avg_duration_ms(&self) -> f64 {
        if self.executions == 0 {
            0.0
        } else {
            (self.total_duration_ns as f64 / self.executions as f64) / 1_000_000.0
        }
    }
}

/// Atomic thread-safe operational telemetry counters for the Knowledge Compiler.
#[derive(Debug, Default)]
pub struct CompilerTelemetry {
    /// Cumulative compilations executed.
    pub total_compilations: AtomicU64,
    /// Cumulative full graph re-compilations executed.
    pub full_compilations: AtomicU64,
    /// Cumulative incremental compilations executed.
    pub incremental_compilations: AtomicU64,
    /// Cumulative entities compiled.
    pub entities_compiled_total: AtomicU64,
    /// Cumulative facts compiled.
    pub facts_compiled_total: AtomicU64,
    /// Cumulative diagnostics emitted.
    pub diagnostics_emitted_total: AtomicU64,
    /// Latency of the last compilation run in nanoseconds.
    pub last_compilation_duration_ns: AtomicU64,
    /// Wall-clock timestamp of the last compilation run in milliseconds.
    pub last_compilation_timestamp_ms: AtomicU64,
    /// Execution mode of the last compilation run.
    pub last_compilation_mode: Mutex<Option<CompilationMode>>,
    /// Per-pass timing metrics indexed by PassId.
    pub pass_metrics: Mutex<BTreeMap<PassId, PassMetrics>>,
}

impl CompilerTelemetry {
    /// Records a compilation run into atomic telemetry counters.
    pub fn record_compilation(
        &self,
        mode: CompilationMode,
        duration_ns: u64,
        timestamp_ms: u64,
        entities_compiled: usize,
        facts_compiled: usize,
        diagnostics_emitted: usize,
        pass_records: &[PassExecutionRecord],
    ) {
        self.total_compilations.fetch_add(1, Ordering::Relaxed);
        match mode {
            CompilationMode::Full => {
                self.full_compilations.fetch_add(1, Ordering::Relaxed);
            }
            CompilationMode::Incremental => {
                self.incremental_compilations
                    .fetch_add(1, Ordering::Relaxed);
            }
        }
        self.entities_compiled_total
            .fetch_add(entities_compiled as u64, Ordering::Relaxed);
        self.facts_compiled_total
            .fetch_add(facts_compiled as u64, Ordering::Relaxed);
        self.diagnostics_emitted_total
            .fetch_add(diagnostics_emitted as u64, Ordering::Relaxed);
        self.last_compilation_duration_ns
            .store(duration_ns, Ordering::Release);
        self.last_compilation_timestamp_ms
            .store(timestamp_ms, Ordering::Release);
        *self.last_compilation_mode.lock().unwrap() = Some(mode);

        let mut metrics_map = self.pass_metrics.lock().unwrap();
        for rec in pass_records {
            let entry = metrics_map
                .entry(rec.pass_id)
                .or_insert_with(|| PassMetrics {
                    pass_name: rec.pass_id.as_str().to_string(),
                    executions: 0,
                    total_duration_ns: 0,
                });
            entry.executions += 1;
            entry.total_duration_ns += rec.duration_ns;
        }
    }
}
