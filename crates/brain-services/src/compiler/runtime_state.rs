//! Compiler runtime state owner, atomic snapshot generator, and report history ring buffer.

use crate::compiler::telemetry::{
    CompilationMode, CompilerTelemetry, PassExecutionRecord, PassMetrics,
};
use brain_integrations::dto::v1::KnowledgeCompilationReport;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Bounded ring buffer holding historical compilation reports (capacity 20).
#[derive(Debug, Clone)]
pub struct CompilationHistory {
    reports: VecDeque<KnowledgeCompilationReport>,
    capacity: usize,
}

impl Default for CompilationHistory {
    fn default() -> Self {
        Self::new(20)
    }
}

impl CompilationHistory {
    /// Creates a new `CompilationHistory` with given report capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            reports: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Pushes a compilation report into history, evicting the oldest report if capacity is exceeded.
    pub fn push(&mut self, report: KnowledgeCompilationReport) {
        if self.reports.len() >= self.capacity {
            self.reports.pop_front();
        }
        self.reports.push_back(report);
    }

    /// Returns the most recent compilation report, if any.
    pub fn latest(&self) -> Option<KnowledgeCompilationReport> {
        self.reports.back().cloned()
    }

    /// Returns a historical report by execution ID, if found.
    pub fn get_by_id(&self, compilation_id: &str) -> Option<KnowledgeCompilationReport> {
        self.reports
            .iter()
            .find(|r| r.compilation_id == compilation_id)
            .cloned()
    }

    /// Returns all cached historical reports.
    pub fn all(&self) -> Vec<KnowledgeCompilationReport> {
        self.reports.iter().cloned().collect()
    }
}

/// Point-in-time immutable atomic snapshot of all compiler operational telemetry and graph version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompilerSnapshot {
    /// Current graph version epoch sequence.
    pub graph_version: u64,
    /// Total compilations executed.
    pub total_compilations: u64,
    /// Total full graph re-compilations executed.
    pub full_compilations: u64,
    /// Total incremental compilations executed.
    pub incremental_compilations: u64,
    /// Total entities compiled across runs.
    pub entities_compiled_total: u64,
    /// Total facts compiled across runs.
    pub facts_compiled_total: u64,
    /// Total diagnostics emitted across runs.
    pub diagnostics_emitted_total: u64,
    /// Duration of last compilation run in milliseconds, if any.
    pub last_compilation_duration_ms: Option<u64>,
    /// Wall-clock timestamp of last compilation run in milliseconds, if any.
    pub last_compilation_timestamp_ms: Option<u64>,
    /// Mode of last compilation run ("full" or "incremental").
    pub last_compilation_mode: Option<CompilationMode>,
    /// Current background scheduler state machine status ("idle", "waiting", "compiling", etc.).
    pub scheduler_state: String,
    /// Pending coalesced dirty event keys count.
    pub pending_dirty_count: usize,
    /// Per-pass performance metrics list.
    pub pass_metrics: Vec<PassMetrics>,
}

/// Central state owner for compiler telemetry, graph versioning, dirty buffer, and report history.
pub struct CompilerRuntimeState {
    graph_version: AtomicU64,
    telemetry: Arc<CompilerTelemetry>,
    history: Mutex<CompilationHistory>,
    dirty_buffer: Arc<crate::compiler::scheduler::CoalescingDirtyBuffer>,
    scheduler_state: Arc<Mutex<crate::compiler::scheduler::SchedulerState>>,
}

impl Default for CompilerRuntimeState {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerRuntimeState {
    /// Instantiates a new `CompilerRuntimeState`.
    pub fn new() -> Self {
        Self {
            graph_version: AtomicU64::new(1),
            telemetry: Arc::new(CompilerTelemetry::default()),
            history: Mutex::new(CompilationHistory::default()),
            dirty_buffer: Arc::new(crate::compiler::scheduler::CoalescingDirtyBuffer::new(1)),
            scheduler_state: Arc::new(Mutex::new(crate::compiler::scheduler::SchedulerState::Idle)),
        }
    }

    /// Returns a reference to the thread-shared dirty key coalescing buffer.
    pub fn dirty_buffer(&self) -> Arc<crate::compiler::scheduler::CoalescingDirtyBuffer> {
        Arc::clone(&self.dirty_buffer)
    }

    /// Increments the graph version epoch counter and returns the new version.
    pub fn increment_graph_version(&self) -> u64 {
        self.graph_version.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the current graph version epoch sequence.
    pub fn graph_version(&self) -> u64 {
        self.graph_version.load(Ordering::Acquire)
    }

    /// Returns a reference to the compiler telemetry counters.
    pub fn telemetry(&self) -> Arc<CompilerTelemetry> {
        Arc::clone(&self.telemetry)
    }

    /// Records a completed compilation run into telemetry and history ring buffer.
    pub fn record_compilation(
        &self,
        mode: CompilationMode,
        report: &KnowledgeCompilationReport,
        pass_records: &[PassExecutionRecord],
    ) {
        self.telemetry.record_compilation(
            mode,
            report.duration_ms * 1_000_000,
            report.timestamp_ms,
            report.entities_compiled,
            report.facts_compiled,
            report.diagnostics.len(),
            pass_records,
        );

        self.history.lock().unwrap().push(report.clone());
    }

    /// Returns an atomic point-in-time snapshot of compiler telemetry and status.
    pub fn live_snapshot(&self) -> CompilerSnapshot {
        let last_dur_ns = self
            .telemetry
            .last_compilation_duration_ns
            .load(Ordering::Acquire);
        let last_ts_ms = self
            .telemetry
            .last_compilation_timestamp_ms
            .load(Ordering::Acquire);
        let mode = *self.telemetry.last_compilation_mode.lock().unwrap();

        let pass_metrics_list: Vec<PassMetrics> = self
            .telemetry
            .pass_metrics
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();

        CompilerSnapshot {
            graph_version: self.graph_version.load(Ordering::Acquire),
            total_compilations: self.telemetry.total_compilations.load(Ordering::Acquire),
            full_compilations: self.telemetry.full_compilations.load(Ordering::Acquire),
            incremental_compilations: self
                .telemetry
                .incremental_compilations
                .load(Ordering::Acquire),
            entities_compiled_total: self
                .telemetry
                .entities_compiled_total
                .load(Ordering::Acquire),
            facts_compiled_total: self.telemetry.facts_compiled_total.load(Ordering::Acquire),
            diagnostics_emitted_total: self
                .telemetry
                .diagnostics_emitted_total
                .load(Ordering::Acquire),
            last_compilation_duration_ms: if last_dur_ns > 0 {
                Some(last_dur_ns / 1_000_000)
            } else {
                None
            },
            last_compilation_timestamp_ms: if last_ts_ms > 0 {
                Some(last_ts_ms)
            } else {
                None
            },
            last_compilation_mode: mode,
            scheduler_state: self.scheduler_state.lock().unwrap().to_string(),
            pending_dirty_count: self.dirty_buffer.pending_count(),
            pass_metrics: pass_metrics_list,
        }
    }

    /// Retrieves historical compilation reports from history ring buffer.
    pub fn compilation_history(&self) -> Vec<KnowledgeCompilationReport> {
        self.history.lock().unwrap().all()
    }

    /// Retrieves the most recent compilation report from history ring buffer.
    pub fn latest_report(&self) -> Option<KnowledgeCompilationReport> {
        self.history.lock().unwrap().latest()
    }
}
