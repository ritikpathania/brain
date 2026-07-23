//! Event-driven background CompilerScheduler, CoalescingDirtyBuffer, and CompilerSchedulingPolicy (KPP v1.5).

use crate::compiler::dirty_set::DirtySet;
use crate::compiler::ir::{EntityId, FactId};
use crate::compiler::pass::CompilerContext;
use crate::compiler::telemetry::CompilationMode;
use crate::compiler::KnowledgeCompiler;
use brain_domain::SessionId;
use brain_integrations::dto::v1::KnowledgeCompilationReport;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// State machine enum for background CompilerScheduler execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SchedulerState {
    /// Background scheduler task is not running.
    Stopped,
    /// Background task is idle, waiting for dirty events or timer tick.
    Idle,
    /// Dirty events present below trigger threshold, accumulating coalesced updates.
    Waiting,
    /// Incremental or full compilation cycle active in KnowledgeCompiler.
    Compiling,
    /// Finalizing cycle and producing CompilationResult for application runtime.
    Finalizing,
    /// Executing startup recovery or epoch alignment check.
    Recovering,
}

impl std::fmt::Display for SchedulerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchedulerState::Stopped => write!(f, "stopped"),
            SchedulerState::Idle => write!(f, "idle"),
            SchedulerState::Waiting => write!(f, "waiting"),
            SchedulerState::Compiling => write!(f, "compiling"),
            SchedulerState::Finalizing => write!(f, "finalizing"),
            SchedulerState::Recovering => write!(f, "recovering"),
        }
    }
}

/// Explicit decision returned by `CompilerSchedulingPolicy` evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileDecision {
    /// Trigger compilation now with the given mode.
    CompileNow {
        /// Mode of compilation to trigger ("full" or "incremental").
        mode: CompilationMode,
    },
    /// Wait for more dirty events or timer tick.
    Wait {
        /// Number of pending dirty events currently queued.
        pending_count: usize,
    },
    /// Force full graph re-compilation due to version epoch skew.
    ForceFull,
}

/// Configurable scheduling policy parameters for background compilation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilerSchedulerConfig {
    /// Flag indicating if continuous background scheduling is enabled.
    pub background_enabled: bool,
    /// Maximum quiet window interval in seconds before forcing compilation cycle.
    pub interval_secs: u64,
    /// Minimum dirty events count to trigger immediate compilation cycle.
    pub min_dirty_events_trigger: usize,
    /// Maximum batch size of dirty entities per compilation cycle.
    pub max_batch_size: usize,
    /// Maximum execution time budget per cycle in milliseconds.
    pub cycle_time_budget_ms: u64,
}

impl Default for CompilerSchedulerConfig {
    fn default() -> Self {
        Self {
            background_enabled: true,
            interval_secs: 5,
            min_dirty_events_trigger: 3,
            max_batch_size: 1000,
            cycle_time_budget_ms: 10000,
        }
    }
}

/// Pure scheduling policy evaluator determining when compilation should occur.
#[derive(Debug, Clone)]
pub struct CompilerSchedulingPolicy {
    /// Configuration rules for the scheduling policy.
    pub config: CompilerSchedulerConfig,
}

impl CompilerSchedulingPolicy {
    /// Instantiates a new policy with given configuration.
    pub fn new(config: CompilerSchedulerConfig) -> Self {
        Self { config }
    }

    /// Evaluates dirty buffer depth and time since last compile to make a compilation decision.
    pub fn evaluate(
        &self,
        pending_events: usize,
        last_compile_timestamp_ms: u64,
        graph_version_mismatch: bool,
    ) -> CompileDecision {
        if !self.config.background_enabled {
            return CompileDecision::Wait {
                pending_count: pending_events,
            };
        }

        if graph_version_mismatch {
            return CompileDecision::ForceFull;
        }

        if pending_events == 0 {
            return CompileDecision::Wait { pending_count: 0 };
        }

        if pending_events >= self.config.min_dirty_events_trigger {
            return CompileDecision::CompileNow {
                mode: CompilationMode::Incremental,
            };
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let elapsed_secs = if last_compile_timestamp_ms > 0 && now_ms > last_compile_timestamp_ms {
            (now_ms - last_compile_timestamp_ms) / 1000
        } else {
            0
        };

        if elapsed_secs >= self.config.interval_secs {
            CompileDecision::CompileNow {
                mode: CompilationMode::Incremental,
            }
        } else {
            CompileDecision::Wait {
                pending_count: pending_events,
            }
        }
    }
}

/// Thread-safe thread-shared dirty key coalescing buffer for observation updates.
#[derive(Debug)]
pub struct CoalescingDirtyBuffer {
    /// Graph version epoch associated with this dirty buffer.
    pub graph_version: AtomicU64,
    dirty_entities: Mutex<HashSet<EntityId>>,
    dirty_facts: Mutex<HashSet<FactId>>,
    pending_count: AtomicUsize,
}

impl Default for CoalescingDirtyBuffer {
    fn default() -> Self {
        Self::new(1)
    }
}

impl CoalescingDirtyBuffer {
    /// Instantiates a new dirty buffer for given graph version epoch.
    pub fn new(graph_version: u64) -> Self {
        Self {
            graph_version: AtomicU64::new(graph_version),
            dirty_entities: Mutex::new(HashSet::new()),
            dirty_facts: Mutex::new(HashSet::new()),
            pending_count: AtomicUsize::new(0),
        }
    }

    /// Marks an entity ID dirty.
    pub fn mark_entity_dirty(&self, entity_id: EntityId) {
        if self.dirty_entities.lock().unwrap().insert(entity_id) {
            self.pending_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Marks a fact ID dirty.
    pub fn mark_fact_dirty(&self, fact_id: FactId) {
        if self.dirty_facts.lock().unwrap().insert(fact_id) {
            self.pending_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Returns the number of pending coalesced dirty keys.
    pub fn pending_count(&self) -> usize {
        self.pending_count.load(Ordering::Acquire)
    }

    /// Drains dirty keys into an immutable `DirtySet` for pass execution.
    pub fn drain(&self, graph_version: u64) -> DirtySet {
        let mut ds = DirtySet::new(graph_version);
        let mut entities = self.dirty_entities.lock().unwrap();
        let mut facts = self.dirty_facts.lock().unwrap();

        for e in entities.drain() {
            ds.mark_entity(e);
        }
        for f in facts.drain() {
            ds.mark_fact(f);
        }

        self.pending_count.store(0, Ordering::Release);
        self.graph_version.store(graph_version, Ordering::Release);

        ds
    }
}

/// Pure result produced by a successful compilation cycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompilationResult {
    /// Immutable compilation report.
    pub report: KnowledgeCompilationReport,
    /// Mode of compilation execution ("full" or "incremental").
    pub mode: CompilationMode,
    /// Total canonical entities in Knowledge IR after compilation.
    pub compiled_entities_count: usize,
    /// Total canonical facts in Knowledge IR after compilation.
    pub compiled_facts_count: usize,
    /// Graph version epoch sequence.
    pub graph_version: u64,
}

/// Background orchestrator responsible for evaluating policy and triggering compiler cycles.
pub struct CompilerScheduler {
    policy: CompilerSchedulingPolicy,
    state: Arc<Mutex<SchedulerState>>,
    running: Arc<AtomicBool>,
    cancellation_token: CancellationToken,
}

impl CompilerScheduler {
    /// Instantiates a new `CompilerScheduler`.
    pub fn new(config: CompilerSchedulerConfig) -> Self {
        Self {
            policy: CompilerSchedulingPolicy::new(config),
            state: Arc::new(Mutex::new(SchedulerState::Stopped)),
            running: Arc::new(AtomicBool::new(false)),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Returns the current state machine status.
    pub fn state(&self) -> SchedulerState {
        *self.state.lock().unwrap()
    }

    /// Returns the policy configuration.
    pub fn config(&self) -> &CompilerSchedulerConfig {
        &self.policy.config
    }

    /// Cancels background loop gracefully.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Release);
        self.cancellation_token.cancel();
        *self.state.lock().unwrap() = SchedulerState::Stopped;
    }

    /// Executes a single orchestration step using compiler and buffer.
    pub fn run_step(
        &self,
        compiler: &KnowledgeCompiler,
        buffer: &CoalescingDirtyBuffer,
        ir: &mut crate::compiler::ir::KnowledgeIR,
    ) -> Option<CompilationResult> {
        let pending = buffer.pending_count();
        let last_ts = compiler
            .runtime_state()
            .live_snapshot()
            .last_compilation_timestamp_ms
            .unwrap_or(0);
        let version_mismatch = buffer.graph_version.load(Ordering::Acquire)
            != compiler.runtime_state().graph_version();

        let decision = self.policy.evaluate(pending, last_ts, version_mismatch);

        match decision {
            CompileDecision::Wait { .. } => {
                let mut st = self.state.lock().unwrap();
                if pending > 0 {
                    *st = SchedulerState::Waiting;
                } else {
                    *st = SchedulerState::Idle;
                }
                None
            }
            CompileDecision::CompileNow { mode } => self.execute_cycle(compiler, buffer, ir, mode),
            CompileDecision::ForceFull => {
                self.execute_cycle(compiler, buffer, ir, CompilationMode::Full)
            }
        }
    }

    fn execute_cycle(
        &self,
        compiler: &KnowledgeCompiler,
        buffer: &CoalescingDirtyBuffer,
        ir: &mut crate::compiler::ir::KnowledgeIR,
        mode: CompilationMode,
    ) -> Option<CompilationResult> {
        *self.state.lock().unwrap() = SchedulerState::Compiling;

        let version = compiler.runtime_state().graph_version();
        let dirty_set = buffer.drain(version);

        let context = CompilerContext {
            compilation_id: Uuid::new_v4(),
            session_id: SessionId::new(),
            graph_version: version,
            dirty_set: None,
            min_confidence_threshold: 0.70,
            time_budget_ms: self.policy.config.cycle_time_budget_ms,
            cancellation_token: self.cancellation_token.clone(),
            config: crate::compiler::config::CompilerOptimizationConfig::default(),
        };

        let (compiled_ir, report) = if mode == CompilationMode::Full || dirty_set.is_full_recompile
        {
            compiler.compile(&context, ir)
        } else {
            compiler.compile_incremental(&context, ir, dirty_set)
        };

        *self.state.lock().unwrap() = SchedulerState::Finalizing;

        let result = CompilationResult {
            report,
            mode,
            compiled_entities_count: compiled_ir.entities.len(),
            compiled_facts_count: compiled_ir.facts.len(),
            graph_version: version,
        };

        *self.state.lock().unwrap() = SchedulerState::Idle;
        Some(result)
    }
}
