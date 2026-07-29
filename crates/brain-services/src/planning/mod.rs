//! Planning Runtime & Goal Decomposition Engine (Phase 7 Milestone 7.1).
//!
//! Provides `GoalIntent`, intermediate `PlanningIR`, immutable `TaskPlan` artifact compilation, pure `GoalValidator`,
//! and orchestration root `PlanningRuntime`.

pub mod checkpoint_migrator;
pub mod checkpoint_store;
pub mod cluster;
pub mod cluster_projection;
pub mod compiler;
pub mod control_plane_projection;
pub mod coordinator;
pub mod decomposer;
pub mod dispatcher;
pub mod event_publisher;
pub mod execution_monitor;
pub mod execution_plan;
pub mod execution_planner;
pub mod execution_runtime;
pub mod fenced_lease;
pub mod global_scheduler;
pub mod heartbeat;
pub mod leadership_projection;
pub mod lease_recovery;
pub mod models;
pub mod optimizer;
pub mod planning_runtime;
pub mod retry_policy;
pub mod scheduler;
pub mod scheduling_metrics;
pub mod supervision;
pub mod supervision_replay;
pub mod validator;
pub mod worker_registry;
pub mod worker_session;

pub use checkpoint_migrator::{CheckpointMigrator, DefaultCheckpointMigrator};
pub use checkpoint_store::{CheckpointStore, InMemoryCheckpointStore};
pub use cluster::{
    ClusterError, ClusterEvent, ClusterEventId, ClusterEventKind, ClusterManager, ClusterNode,
    ClusterNodeRole, ClusterNodeStatus, EpochId, NodeAddress, NodeId,
};
pub use cluster_projection::ClusterTopologyProjection;
pub use compiler::TaskPlanCompiler;
pub use control_plane_projection::ClusterControlPlaneProjection;
pub use coordinator::{
    CoordinatorElectionEngine, CoordinatorLeader, LeaderElectionStrategy, LeadershipEvent,
    LeadershipEventId, LeadershipEventKind, LeadershipState, SingleCoordinatorStrategy,
    StaticLeaderStrategy, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
pub use decomposer::GoalDecomposer;
pub use dispatcher::{
    DeliveryAck, DispatchLifecycleEvent, DispatchLifecycleEventId, DispatchLifecycleEventKind,
    ExecutionDispatcher, LocalExecutionDispatcher, RemoteExecutionDispatcher,
};
pub use event_publisher::{EventPublishError, EventPublisher, InMemoryEventPublisher};
pub use execution_monitor::{ExecutionMetricsSnapshot, ExecutionMonitor};
pub use execution_plan::{
    BarrierKind, ExecutionPlan, ExecutionPlanId, ExecutionPlanningError, ExecutionPlanningPolicy,
    ExecutionStage, SchedulingStrategy,
};
pub use execution_planner::ExecutionPlanner;
pub use execution_runtime::{
    DefaultTaskExecutor, ExecutionFailure, ExecutionFailureKind, ExecutionId, ExecutionReport,
    ExecutionState, TaskExecutionEvent, TaskExecutionEventKind, TaskExecutionRecord,
    TaskExecutionRuntime, TaskExecutionStatus, TaskExecutor,
};
pub use fenced_lease::FencedLease;
pub use global_scheduler::GlobalScheduler;
pub use heartbeat::{HeartbeatPolicy, WorkerHeartbeatService};
pub use leadership_projection::LeadershipProjection;
pub use lease_recovery::{
    HeartbeatGracePolicy, ImmediateReassignPolicy, LeaseRecoveryEngine, RecoveryAction,
    RecoveryContext, RecoveryPolicy,
};
pub use models::{
    CapabilityId, Constraint, GoalId, GoalIntent, PlanId, PlanningIR, PlanningValidationError,
    PlanningValidationKind, PlanningValidationReport, Priority, TaskCandidate, TaskDependencyEdge,
    TaskGraph, TaskId, TaskPlan, TaskStep,
};
pub use optimizer::{
    BranchConsolidationPass, ConfidenceMaximizationPass, OptimizationPass, OptimizationPolicy,
    OptimizationReport, OptimizationTransformation, PlanOptimizer, RedundantTaskEliminationPass,
};
pub use planning_runtime::PlanningRuntime;
pub use retry_policy::{BackoffStrategy, DefaultRetryClassifier, RetryClassifier, RetryPolicy};
pub use scheduler::{
    ExecutionScheduler, LeaseId, LeaseState, LeastBusyPolicy, RoundRobinPolicy, SchedulerError,
    SchedulingEvent, SchedulingEventId, SchedulingEventKind, SchedulingPolicy, TaskAssignment,
    WorkerLease,
};
pub use scheduling_metrics::SchedulingMetricsProjection;
pub use supervision::{
    CheckpointCapability, CheckpointCapabilitySet, CheckpointId, ExecutionCheckpoint,
    ExecutionSupervisor, RecoveryReport, SupervisionError, SupervisionEvent, SupervisionEventId,
    SupervisionEventKind, SupervisionState,
};
pub use supervision_replay::{
    AuditEntry, CapabilityCompatibility, CapabilityNegotiator, DynamicProjectionRegistry,
    ProjectionId, SupervisionAuditProjection, SupervisionMetricsProjection, SupervisionProjection,
    SupervisionProjectionEngine, SupervisionStateProjection,
};
pub use validator::GoalValidator;
pub use worker_registry::{
    ExecutionWorker, WorkerId, WorkerRegistry, WorkerRegistryError, WorkerStatus,
};
pub use worker_session::{
    ProtocolNegotiation, SessionId, SessionState, WorkerSession, WorkerSessionManager,
};

/// Supervision engine, stage checkpointing, and resumable recovery for reflection DAGs.
pub mod reflection_supervisor;
pub use reflection_supervisor::*;
