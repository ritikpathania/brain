//! Planning Runtime & Goal Decomposition Engine (Phase 7 Milestone 7.1).
//!
//! Provides `GoalIntent`, intermediate `PlanningIR`, immutable `TaskPlan` artifact compilation, pure `GoalValidator`,
//! and orchestration root `PlanningRuntime`.

pub mod checkpoint_migrator;
pub mod checkpoint_store;
pub mod cluster;
pub mod cluster_bootstrap;
pub mod cluster_configuration;
pub mod cluster_projection;
pub mod cluster_telemetry;
pub mod compiler;
pub mod consensus;
pub mod consensus_storage;
pub mod control_plane_projection;
pub mod coordinator;
pub mod coordinator_failover;
pub mod decomposer;
pub mod dispatcher;
pub mod durable_event_store;
pub mod event_codec;
pub mod event_publisher;
pub mod execution_monitor;
pub mod execution_plan;
pub mod execution_planner;
pub mod execution_runtime;
pub mod fault_injection;
pub mod fenced_lease;
pub mod global_scheduler;
pub mod heartbeat;
pub mod leader_lease_validator;
pub mod leadership_projection;
pub mod lease_recovery;
pub mod linearizable_read_engine;
pub mod log_compactor;
pub mod log_replay_engine;
pub mod models;
pub mod network_transport;
pub mod optimizer;
pub mod planning_runtime;
pub mod read_events;
pub mod read_policy;
pub mod read_projection;
pub mod recovery_engine;
pub mod replication_events;
pub mod replication_flow_controller;
pub mod replication_projection;
pub mod replication_worker;
pub mod retry_policy;
pub mod scheduler;
pub mod scheduling_metrics;
pub mod snapshot_replicator;
pub mod snapshot_restore_engine;
pub mod snapshot_store;
pub mod snapshot_transport;
pub mod sqlite_event_log;
pub mod supervision;
pub mod supervision_replay;
pub mod transport_framing;
pub mod validator;
pub mod worker_registry;
pub mod worker_session;

pub use checkpoint_migrator::{CheckpointMigrator, DefaultCheckpointMigrator};
pub use checkpoint_store::{CheckpointStore, InMemoryCheckpointStore};
pub use cluster::{
    ClusterError, ClusterEvent, ClusterEventId, ClusterEventKind, ClusterManager, ClusterNode,
    ClusterNodeRole, ClusterNodeStatus, EpochId, NodeAddress, NodeId,
};
pub use cluster_bootstrap::{
    BootstrapReport, CliClusterController, ClusterBootstrapEngine, ClusterConfigError,
    ClusterConfigValidator, ClusterNodeConfig, ClusterStatusReport, MembershipChangePlan,
    SnapshotTriggerPlan, ValidatedClusterConfig,
};
pub use cluster_configuration::{
    ConfigurationApplier, ConfigurationPlanner, ConfigurationTransition, ConfigurationVersion,
    MembershipView,
};
pub use cluster_projection::ClusterTopologyProjection;
pub use cluster_telemetry::{
    ClusterHealthDashboard, ClusterTelemetryMetrics, ClusterTelemetryProjection, SlaSloMonitor,
    SloEvaluationReport, SloPolicy,
};
pub use compiler::TaskPlanCompiler;
pub use consensus::{
    AppendEntriesRejectReason, AppendEntriesRequest, AppendEntriesResponse, ConsensusEngine,
    ConsensusError, ConsensusProtocol, ConsensusRole, ConsensusState, InstallSnapshotRequest,
    InstallSnapshotResponse, LeaderLease, LogReplicationState, QuorumEvaluator,
    RaftConsensusStrategy, ReadIndexRequest, ReadIndexResponse, ReadValidationResult, TermId,
    VoteResult,
};
pub use consensus_storage::{ConsensusPersistentState, ConsensusStorage, InMemoryConsensusStorage};
pub use control_plane_projection::ClusterControlPlaneProjection;
pub use coordinator::{
    CoordinatorElectionEngine, CoordinatorLeader, LeaderElectionStrategy, LeadershipEvent,
    LeadershipEventId, LeadershipEventKind, LeadershipState, SingleCoordinatorStrategy,
    StaticLeaderStrategy, LEADERSHIP_EVENT_SCHEMA_VERSION,
};
pub use coordinator_failover::{
    FailoverExecutor, FailoverPlan, FailoverPlanner, FailoverState, FailureDetector,
    RecoveryProgress, RecoveryStrategy, StateRecoveryReport,
};
pub use decomposer::GoalDecomposer;
pub use dispatcher::{
    DeliveryAck, DispatchLifecycleEvent, DispatchLifecycleEventId, DispatchLifecycleEventKind,
    ExecutionDispatcher, LocalExecutionDispatcher, RemoteExecutionDispatcher,
};
pub use durable_event_store::{EventEnvelope, EventLog, InMemoryEventLog, SequenceNumber};
pub use event_codec::{EventCodec, JsonEventCodec};
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
pub use fault_injection::{
    FaultEffects, FaultInjectionHarness, FaultInjector, NetworkDelaySimulator, PacketDropSimulator,
    PartitionSimulator,
};
pub use fenced_lease::FencedLease;
pub use global_scheduler::GlobalScheduler;
pub use heartbeat::{HeartbeatPolicy, WorkerHeartbeatService};
pub use leader_lease_validator::LeaderLeaseValidator;
pub use leadership_projection::LeadershipProjection;
pub use lease_recovery::{
    HeartbeatGracePolicy, ImmediateReassignPolicy, LeaseRecoveryEngine, RecoveryAction,
    RecoveryContext, RecoveryPolicy,
};
pub use linearizable_read_engine::{
    LinearizableReadEngine, ReadConsistencyStrategy, ReadPlan, ReadPlanKind, ReadPlanner,
};
pub use log_compactor::{CompactionExecutor, CompactionPlan, CompactionPlanner};
pub use log_replay_engine::{LogReplayEngine, ReplayTarget};
pub use models::{
    CapabilityId, Constraint, GoalId, GoalIntent, PlanId, PlanningIR, PlanningValidationError,
    PlanningValidationKind, PlanningValidationReport, Priority, TaskCandidate, TaskDependencyEdge,
    TaskGraph, TaskId, TaskPlan, TaskStep,
};
pub use network_transport::{
    ConnectionStatus, GrpcSnapshotTransport, QuicSnapshotTransport, TransportConnectionPool,
};
pub use optimizer::{
    BranchConsolidationPass, ConfidenceMaximizationPass, OptimizationPass, OptimizationPolicy,
    OptimizationReport, OptimizationTransformation, PlanOptimizer, RedundantTaskEliminationPass,
};
pub use planning_runtime::PlanningRuntime;
pub use read_events::{
    ReadEvent, ReadEventId, ReadEventKind, ReadEventPublisher, READ_EVENT_SCHEMA_VERSION,
};
pub use read_policy::{LeasePriorityPolicy, QuorumOnlyPolicy, ReadPolicy, ReadPolicyEvaluator};
pub use read_projection::{ReadMetrics, ReadProjection};
pub use recovery_engine::{RecoveryEngine, RestoreFromSnapshot};
pub use replication_events::{
    ReplicationEvent, ReplicationEventId, ReplicationEventKind, ReplicationEventPublisher,
    REPLICATION_EVENT_SCHEMA_VERSION,
};
pub use replication_flow_controller::{
    FlowDecision, ReplicationFlowController, ReplicationMeasurements,
};
pub use replication_projection::{
    FollowerReplicationMetrics, ReplicationHealth, ReplicationHealthEvaluator,
    ReplicationProjection,
};
pub use replication_worker::{
    ReplicationBatch, ReplicationBatchKind, ReplicationCoordinator, ReplicationTask,
    ReplicationWorker, ReplicationWorkerState,
};
pub use retry_policy::{BackoffStrategy, DefaultRetryClassifier, RetryClassifier, RetryPolicy};
pub use scheduler::{
    ExecutionScheduler, LeaseId, LeaseState, LeastBusyPolicy, RoundRobinPolicy, SchedulerError,
    SchedulingEvent, SchedulingEventId, SchedulingEventKind, SchedulingPolicy, TaskAssignment,
    WorkerLease,
};
pub use scheduling_metrics::SchedulingMetricsProjection;
pub use snapshot_replicator::{
    SnapshotChunk, SnapshotReplicationPlanner, SnapshotReplicator, SnapshotTransferId,
    SnapshotTransferPlan, SnapshotTransferState,
};
pub use snapshot_restore_engine::SnapshotRestoreEngine;
pub use snapshot_store::{
    InMemorySnapshotStore, JsonSnapshotCodec, LogSnapshot, SnapshotBuilder, SnapshotCodec,
    SnapshotStore,
};
pub use snapshot_transport::{ChunkedStreamAdapter, MockSnapshotTransport, SnapshotTransport};
pub use sqlite_event_log::SqliteEventLog;
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
pub use transport_framing::{
    FramingError, IntegrityPolicy, MessageFramingCodec, TransportFrameHeader, MAX_FRAME_SIZE,
    TRANSPORT_FRAME_MAGIC, TRANSPORT_FRAME_SCHEMA_VERSION,
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
