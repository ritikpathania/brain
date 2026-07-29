//! Supervision Event Replay Engine (`SupervisionProjectionEngine`) and Worker Capability Negotiator (`CapabilityNegotiator`) (Phase 8 Milestone 8.4).
//!
//! ### Architectural Invariants:
//! 1. Open for Extension: Projections implement `SupervisionProjection` trait (`apply_event(&mut self, event: &SupervisionEvent)`).
//! 2. Replay Framework: `SupervisionProjectionEngine` drives event replay across any arbitrary registered projections cleanly.
//! 3. Structured Audit Entries: `SupervisionAuditProjection` records structured `AuditEntry` items.
//! 4. Diagnostic Capability Compatibility: `CapabilityNegotiator` provides explicit diagnostic feedback (`MissingCapabilities`) when a worker lacks required capabilities.

use crate::planning::supervision::{
    CheckpointCapability, CheckpointCapabilitySet, CheckpointId, SupervisionEvent,
    SupervisionEventId, SupervisionEventKind, SupervisionState,
};
use serde::{Deserialize, Serialize};

/// Trait implemented by event-sourced supervision projections.
pub trait SupervisionProjection: Send + Sync {
    /// Applies a single `SupervisionEvent` to update projection state.
    fn apply_event(&mut self, event: &SupervisionEvent);
}

/// Structured projection reconstructing control plane `SupervisionState`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisionStateProjection {
    /// Reconstructed supervision state.
    pub state: SupervisionState,
    /// Currently active checkpoint ID if restored or created.
    pub active_checkpoint_id: Option<CheckpointId>,
    /// Last processed event ID.
    pub last_event_id: Option<SupervisionEventId>,
    /// Count of events processed during replay.
    pub events_processed_count: usize,
}

impl Default for SupervisionStateProjection {
    fn default() -> Self {
        Self {
            state: SupervisionState::Active,
            active_checkpoint_id: None,
            last_event_id: None,
            events_processed_count: 0,
        }
    }
}

impl SupervisionProjection for SupervisionStateProjection {
    fn apply_event(&mut self, event: &SupervisionEvent) {
        self.last_event_id = Some(event.event_id);
        self.events_processed_count += 1;

        match event.kind {
            SupervisionEventKind::ExecutionPaused => {
                self.state = SupervisionState::Paused;
            }
            SupervisionEventKind::ExecutionResumed => {
                self.state = SupervisionState::Active;
            }
            SupervisionEventKind::ExecutionCancelled => {
                self.state = SupervisionState::Cancelled;
            }
            SupervisionEventKind::CheckpointCreated | SupervisionEventKind::CheckpointRestored => {
                self.state = SupervisionState::Checkpointed;
            }
            SupervisionEventKind::RecoveryStarted => {
                self.state = SupervisionState::Recovering;
            }
            SupervisionEventKind::RecoveryCompleted => {
                self.state = SupervisionState::Active;
            }
        }
    }
}

/// Operational telemetry projection tracking supervision event metrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SupervisionMetricsProjection {
    /// Total events processed.
    pub total_events: usize,
    /// Checkpoints created count.
    pub checkpoints_created_count: usize,
    /// Recovery operations executed count.
    pub recoveries_count: usize,
    /// Pauses executed count.
    pub pauses_count: usize,
}

impl SupervisionProjection for SupervisionMetricsProjection {
    fn apply_event(&mut self, event: &SupervisionEvent) {
        self.total_events += 1;
        match event.kind {
            SupervisionEventKind::CheckpointCreated => {
                self.checkpoints_created_count += 1;
            }
            SupervisionEventKind::RecoveryStarted => {
                self.recoveries_count += 1;
            }
            SupervisionEventKind::ExecutionPaused => {
                self.pauses_count += 1;
            }
            _ => {}
        }
    }
}

/// Structured audit entry tracking a single supervisory control operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Event timestamp in milliseconds.
    pub timestamp_ms: u64,
    /// Target supervision event ID.
    pub event_id: SupervisionEventId,
    /// Event classification kind.
    pub kind: SupervisionEventKind,
    /// Descriptive event text.
    pub message: String,
}

/// Human-readable and structured audit projection tracking control plane history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SupervisionAuditProjection {
    /// Structured audit entries.
    pub entries: Vec<AuditEntry>,
}

impl SupervisionProjection for SupervisionAuditProjection {
    fn apply_event(&mut self, event: &SupervisionEvent) {
        self.entries.push(AuditEntry {
            timestamp_ms: event.timestamp_ms,
            event_id: event.event_id,
            kind: event.kind,
            message: event.message.clone(),
        });
    }
}

/// Strongly-typed projection identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProjectionId(pub String);

impl std::fmt::Display for ProjectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proj_{}", self.0)
    }
}

/// Dynamic registry allowing runtime registration and event dispatching across projections.
#[derive(Default)]
pub struct DynamicProjectionRegistry {
    projections: std::collections::HashMap<ProjectionId, Box<dyn SupervisionProjection>>,
}

impl DynamicProjectionRegistry {
    /// Instantiates a new `DynamicProjectionRegistry`.
    pub fn new() -> Self {
        Self {
            projections: std::collections::HashMap::new(),
        }
    }

    /// Registers a new `SupervisionProjection` under specified `ProjectionId`.
    pub fn register(&mut self, id: ProjectionId, projection: Box<dyn SupervisionProjection>) {
        self.projections.insert(id, projection);
    }

    /// Dispatches a `SupervisionEvent` to all dynamically registered projections.
    pub fn dispatch_event(&mut self, event: &SupervisionEvent) {
        for proj in self.projections.values_mut() {
            proj.apply_event(event);
        }
    }
}

/// Engine driving event replay across registered projections.
#[derive(Debug, Clone, Default)]
pub struct SupervisionProjectionEngine;

impl SupervisionProjectionEngine {
    /// Instantiates a new `SupervisionProjectionEngine`.
    pub fn new() -> Self {
        Self
    }

    /// Drives event replay across a slice of mutable `SupervisionProjection` trait objects.
    pub fn drive_projections(
        &self,
        events: &[SupervisionEvent],
        projections: &mut [&mut dyn SupervisionProjection],
    ) {
        for event in events {
            for proj in projections.iter_mut() {
                proj.apply_event(event);
            }
        }
    }
}

/// Diagnostic compatibility result produced by capability negotiation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityCompatibility {
    /// Worker supports all required checkpoint capabilities.
    Compatible,
    /// Worker is missing specific required capabilities.
    MissingCapabilities(Vec<CheckpointCapability>),
}

/// Negotiator checking compatibility between worker capabilities and checkpoint requirements.
#[derive(Debug, Clone, Default)]
pub struct CapabilityNegotiator;

impl CapabilityNegotiator {
    /// Checks if supported worker capabilities satisfy required checkpoint capabilities.
    pub fn check_compatibility(
        supported: &CheckpointCapabilitySet,
        required: &CheckpointCapabilitySet,
    ) -> CapabilityCompatibility {
        let mut missing = Vec::new();

        let all_caps = [
            CheckpointCapability::SupportsStageResume,
            CheckpointCapability::SupportsTaskRetry,
            CheckpointCapability::SupportsStateReplay,
        ];

        for cap in all_caps {
            if required.has(cap) && !supported.has(cap) {
                missing.push(cap);
            }
        }

        if missing.is_empty() {
            CapabilityCompatibility::Compatible
        } else {
            CapabilityCompatibility::MissingCapabilities(missing)
        }
    }
}
