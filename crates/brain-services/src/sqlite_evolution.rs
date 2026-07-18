use std::sync::Arc;
use std::time::SystemTime;
use std::hash::{Hash, Hasher};
use brain_core::{
    events::{EventSource, OperationId, TaskProgress, TaskState, SemanticStage, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher},
    evolution::{
        Observation, IngestionValidator,
        Canonicalizer, CanonicalizationResult, DomainEventDescriptor, ProjectionInstanceId
    },
    reflection::{ReflectionEngine, ReflectionTarget},
};
use brain_domain::{EpochId, Node, NodeId, NodeKind, GraphProvenance, ProvenanceSource};
use brain_storage::SqliteStorage;
use crate::evolution_service::StandardIngestionValidator;

/// Concrete evolution canonicalization engine backed by SQLite storage.
///
/// The event dispatcher field is typed as `Arc<dyn RuntimeEventDispatcher>` — the concrete
/// implementation (currently `InMemoryEventDispatcher`) is hidden behind the contract.
/// Swap to a different dispatcher by passing a different `Arc<dyn RuntimeEventDispatcher>`.
pub struct SqliteCanonicalizer {
    storage: SqliteStorage,
    event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
    validator: StandardIngestionValidator,
    /// Optional reflection engine. When `Some`, reflection runs after every successful
    /// canonicalization. When `None`, the canonicalizer behaves exactly as in Sprint 2.
    reflection_engine: Option<Arc<dyn ReflectionEngine<Error = brain_core::errors::BrainError>>>,
}

impl SqliteCanonicalizer {
    /// Creates a new `SqliteCanonicalizer` without a reflection engine.
    pub fn new(
        storage: SqliteStorage,
        event_dispatcher: Arc<dyn RuntimeEventDispatcher>,
    ) -> Self {
        Self {
            storage,
            event_dispatcher,
            validator: StandardIngestionValidator,
            reflection_engine: None,
        }
    }

    /// Attaches an optional reflection engine, enabling post-canonicalization reflection.
    ///
    /// ```text
    /// SqliteCanonicalizer::new(storage, dispatcher)
    ///     .with_reflection(Arc::new(InMemoryReflectionEngine::new(dispatcher)))
    /// ```
    pub fn with_reflection(
        mut self,
        engine: Arc<dyn ReflectionEngine<Error = brain_core::errors::BrainError>>,
    ) -> Self {
        self.reflection_engine = Some(engine);
        self
    }
}

impl Canonicalizer for SqliteCanonicalizer {
    type Error = brain_core::errors::BrainError;

    fn canonicalize(&self, obs: Observation) -> Result<CanonicalizationResult, Self::Error> {
        let op_id = OperationId::new_v4();
        let corr_id = obs.provenance.correlation_id;

        // Helper to dispatch progress events
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

        // 4. Run database modifications inside a transaction boundary using abstract Repository traits.
        // CONSTRAINT: Absolutely no SQL queries are executed here.
        let (node_id, next_epoch) = self.storage.run_transaction(|tx| {
            let repos = tx.repositories();
            let configs = repos.configs();

            // Retrieve current epoch from configuration repository
            let epoch_str = configs.get_key("current_epoch")?;
            let current_epoch = match epoch_str {
                Some(s) => {
                    let val = s.parse::<u64>().map_err(|e| brain_core::errors::BrainError::Storage {
                        message: format!("Failed to parse persisted epoch: {}", e),
                        source: None,
                    })?;
                    EpochId(val)
                }
                None => EpochId::initial(),
            };

            // Generate deterministic NodeId based on payload hash using standard DefaultHasher
            let payload_str = String::from_utf8_lossy(&obs.payload);
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            payload_str.hash(&mut hasher);
            let hash_val = hasher.finish();

            let mut bytes = [0u8; 16];
            bytes[0..8].copy_from_slice(&hash_val.to_be_bytes());
            bytes[8..16].copy_from_slice(&hash_val.to_be_bytes());
            let node_id = NodeId(uuid::Uuid::from_bytes(bytes));

            // Construct new node
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

            // Save node and advance epoch via abstract traits
            repos.nodes().save(&node)?;

            let next_epoch = current_epoch.next();
            configs.save_key("current_epoch", &next_epoch.0.to_string())?;

            Ok((node_id, next_epoch))
        })?;

        dispatch_progress(TaskState::Progressing {
            stage: SemanticStage::Synthesis,
            completed_items: Some(1),
            total_items: Some(1),
        }, 7);

        // 5. Build canonicalization results
        let domain_event = DomainEventDescriptor {
            event_type: "MemoryIngested".to_string(),
            timestamp: SystemTime::now(),
        };

        let result = CanonicalizationResult {
            epoch: next_epoch,
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

        // Dispatch invalidation event to subscribers
        self.event_dispatcher.dispatch(Arc::new(ProjectionInstanceInvalidatedEvent {
            projection_type: "MemoryListProjection".to_string(),
            epoch: next_epoch,
            source: EventSource::Ingestion,
            correlation_id: corr_id,
        }));

        dispatch_progress(TaskState::Completed, 9);

        // 7. Optional: Reflection pass over affected entities (Sprint 3+)
        //    When a reflection engine is attached, it runs after the transaction commits.
        //    Sprint 3 is emit-only: the engine dispatches ReflectionCompletedEvent and
        //    per-entity ProjectionInstanceInvalidatedEvents. No graph mutations occur.
        if let Some(ref engine) = self.reflection_engine {
            dispatch_progress(TaskState::Progressing {
                stage: SemanticStage::Reflection,
                completed_items: None,
                total_items: None,
            }, 10);

            let target = ReflectionTarget {
                affected_entities: result.affected_entities.clone(),
                epoch: result.epoch,
                correlation_id: corr_id,
            };

            // Reflection errors are non-fatal: canonicalization already committed successfully.
            let _ = engine.reflect(target);
        }

        Ok(result)
    }
}
