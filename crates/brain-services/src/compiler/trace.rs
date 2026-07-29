//! CompilerTrace primitives separating deterministic structural trace records from non-deterministic performance timings.

use crate::compiler::telemetry::PassId;

/// Deterministic structural trace record for a single compiler pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralTraceRecord {
    /// Pass identifier.
    pub pass_id: PassId,
    /// Number of diagnostics emitted during this pass execution.
    pub diagnostics_emitted: usize,
}

/// Non-deterministic performance measurement record for profiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceRecord {
    /// Pass identifier.
    pub pass_id: PassId,
    /// Pass execution duration in nanoseconds.
    pub duration_ns: u64,
}

/// Aggregated compiler trace containing structural records and performance measurements.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompilerTrace {
    /// Deterministic structural execution records.
    pub structural_records: Vec<StructuralTraceRecord>,
    /// Non-deterministic performance timing measurements.
    pub performance_records: Vec<PerformanceRecord>,
}

impl CompilerTrace {
    /// Instantiates an empty `CompilerTrace`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a pass execution entry into the trace.
    pub fn record_pass(&mut self, pass_id: PassId, diagnostics_emitted: usize, duration_ns: u64) {
        self.structural_records.push(StructuralTraceRecord {
            pass_id,
            diagnostics_emitted,
        });
        self.performance_records.push(PerformanceRecord {
            pass_id,
            duration_ns,
        });
    }
}
