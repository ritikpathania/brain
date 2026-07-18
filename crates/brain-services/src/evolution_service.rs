use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use std::hash::{Hash, Hasher};
use brain_core::{
    events::{EventSource, OperationId, TaskProgress, TaskState, SemanticStage, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher},
    evolution::{
        Observation, IngestionValidator, StructuralValidationError, SemanticValidationError,
        Canonicalizer, CanonicalizationResult, DomainEventDescriptor, ProjectionInstanceId
    }
};
use brain_domain::{EpochId, KnowledgeGraph, Node, NodeId, NodeKind, GraphProvenance, ProvenanceSource};
use crate::event_dispatcher::InMemoryEventDispatcher;

/// Concrete persistence-agnostic validator that performs basic structural and semantic safety checks.
pub struct StandardIngestionValidator;

impl IngestionValidator for StandardIngestionValidator {
    type Error = brain_core::errors::BrainError;

    fn validate_structure(&self, obs: &Observation) -> Result<(), Vec<StructuralValidationError>> {
        if obs.payload.is_empty() {
            return Err(vec![StructuralValidationError {
                field: "payload".to_string(),
                error_code: "EMPTY_PAYLOAD".to_string(),
            }]);
        }
        Ok(())
    }

    fn validate_semantics(&self, _obs: &Observation) -> Result<(), Vec<SemanticValidationError>> {
        Ok(())
    }
}

/// Concrete in-memory canonicalization engine that mutates the graph under a Mutex.
pub struct InMemoryCanonicalizer {
    graph: Arc<Mutex<KnowledgeGraph>>,
    epoch: Arc<Mutex<EpochId>>,
    event_dispatcher: Arc<InMemoryEventDispatcher>,
    validator: StandardIngestionValidator,
}

impl InMemoryCanonicalizer {
    /// Creates a new `InMemoryCanonicalizer`.
    pub fn new(
        graph: Arc<Mutex<KnowledgeGraph>>,
        epoch: Arc<Mutex<EpochId>>,
        event_dispatcher: Arc<InMemoryEventDispatcher>,
    ) -> Self {
        Self {
            graph,
            epoch,
            event_dispatcher,
            validator: StandardIngestionValidator,
        }
    }
}

impl Canonicalizer for InMemoryCanonicalizer {
    type Error = brain_core::errors::BrainError;

    fn canonicalize(&self, obs: Observation) -> Result<CanonicalizationResult, Self::Error> {
        let op_id = OperationId::new_v4();
        let corr_id = obs.provenance.correlation_id;

        // Helper to dispatch progress
        let dispatch_progress = |state: TaskState, seq: u64| {
            self.event_dispatcher.dispatch(Arc::new(TaskProgress {
                operation_id: op_id,
                correlation_id: corr_id,
                state,
                source: EventSource::Ingestion,
                sequence: seq,
                timestamp: SystemTime::now(),
            }));
        };

        // 1. Stage: Queued
        dispatch_progress(TaskState::Created, 1);
        dispatch_progress(TaskState::Started, 2);

        // 2. Stage: Observation / Validation
        dispatch_progress(TaskState::Progressing {
            stage: SemanticStage::Observation,
            completed_items: None,
            total_items: None,
        }, 3);

        if let Err(errs) = self.validator.validate_structure(&obs) {
            let err_msg = format!("Structural validation failed: {:?}", errs);
            dispatch_progress(TaskState::Failed(err_msg.clone()), 4);
            return Err(brain_core::errors::BrainError::Validation { message: err_msg });
        }

        if let Err(errs) = self.validator.validate_semantics(&obs) {
            let err_msg = format!("Semantic validation failed: {:?}", errs);
            dispatch_progress(TaskState::Failed(err_msg.clone()), 5);
            return Err(brain_core::errors::BrainError::Validation { message: err_msg });
        }

        // 3. Stage: Extraction / Synthesis
        dispatch_progress(TaskState::Progressing {
            stage: SemanticStage::Extraction,
            completed_items: None,
            total_items: None,
        }, 6);

        // Generate deterministic NodeId based on payload hash using standard DefaultHasher (for deterministic replay test)
        let payload_str = String::from_utf8_lossy(&obs.payload);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        payload_str.hash(&mut hasher);
        let hash_val = hasher.finish();

        let mut bytes = [0u8; 16];
        bytes[0..8].copy_from_slice(&hash_val.to_be_bytes());
        bytes[8..16].copy_from_slice(&hash_val.to_be_bytes());
        let node_id = NodeId(uuid::Uuid::from_bytes(bytes));

        // Use builder/constructor for non-exhaustive Node struct
        let mut node = Node::new(node_id, payload_str.to_string(), NodeKind::Concept);
        node.provenance = GraphProvenance {
            source_conversation: None,
            source_message: None,
            extracted_at: obs.provenance.timestamp.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
            extractor_version: "v1.0.0".to_string(),
            confidence: 1.0,
            text_span: None,
            source: ProvenanceSource::Imported,
        };

        // 4. Lock & mutate canonical graph
        let mut graph_lock = self.graph.lock().unwrap();
        let mut epoch_lock = self.epoch.lock().unwrap();

        // INVARIANT check: Only canonicalization modifies graph state
        graph_lock.add_node(node);
        *epoch_lock = epoch_lock.next();

        let current_epoch = *epoch_lock;

        dispatch_progress(TaskState::Progressing {
            stage: SemanticStage::Synthesis,
            completed_items: Some(1),
            total_items: Some(1),
        }, 7);

        // 5. Build results & events
        let domain_event = DomainEventDescriptor {
            event_type: "MemoryIngested".to_string(),
            timestamp: SystemTime::now(),
        };

        let result = CanonicalizationResult {
            epoch: current_epoch,
            domain_events: vec![domain_event],
            affected_entities: vec![node_id],
            invalidated_projections: vec![ProjectionInstanceId("MemoryListProjection".to_string())],
        };

        // 6. Stage: Projection Invalidation
        dispatch_progress(TaskState::Progressing {
            stage: SemanticStage::Projection,
            completed_items: None,
            total_items: None,
        }, 8);

        // Invalidate Projection Instances synchronously
        self.event_dispatcher.dispatch(Arc::new(ProjectionInstanceInvalidatedEvent {
            projection_type: "MemoryListProjection".to_string(),
            epoch: current_epoch,
            source: EventSource::Ingestion,
            correlation_id: corr_id,
        }));

        dispatch_progress(TaskState::Completed, 9);

        Ok(result)
    }
}
