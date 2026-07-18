#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::SystemTime;
    use brain_domain::{EpochId, NodeType};
    use brain_core::{
        events::{CorrelationId, ProjectionInstanceInvalidatedEvent, RuntimeEventDispatcher},
        evolution::{Observation, Provenance, Canonicalizer}
    };
    use brain_storage::test_utils::TestStorage;
    use brain_services::{
        SqliteCanonicalizer, SqliteProjector, SqliteProjectionManager,
        MemoryListQuery, MemoryListProjection, InMemoryEventDispatcher
    };

    // --- RUNTIME VALIDATION 1: SQLite In-Memory Database Persistence ---
    #[test]
    fn test_runtime_sqlite_persistence() {
        let test_db = TestStorage::new();
        let dispatcher: Arc<dyn RuntimeEventDispatcher> = Arc::new(InMemoryEventDispatcher::new(10));
        let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher));

        let obs = Observation {
            payload: b"Persistent Concept Node".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_a".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        let result = canonicalizer.canonicalize(obs).unwrap();
        assert_eq!(result.epoch.0, 1);

        // Verify the node is actually written in the DB
        let nodes = test_db.storage().run_transaction(|tx| tx.repositories().nodes().list_all()).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].label, "Persistent Concept Node");
        assert_eq!(nodes[0].node_type, NodeType::Concept);
    }

    // --- RUNTIME VALIDATION 2: Transaction Rollback Correctness ---
    #[test]
    fn test_runtime_transaction_rollback() {
        let test_db = TestStorage::new();
        let dispatcher: Arc<dyn RuntimeEventDispatcher> = Arc::new(InMemoryEventDispatcher::new(10));
        let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher));

        // Observation with empty payload triggers structural validation error
        let obs = Observation {
            payload: b"".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_b".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        let res = canonicalizer.canonicalize(obs);
        assert!(res.is_err());

        // Epoch should remain unchanged (default/initial is 0 or unwritten, check config keys)
        let epoch_key = test_db.storage().run_transaction(|tx| tx.repositories().configs().get_key("current_epoch")).unwrap();
        assert!(epoch_key.is_none(), "Epoch config key should not have been written");

        // Verify no nodes were written to DB
        let nodes = test_db.storage().run_transaction(|tx| tx.repositories().nodes().list_all()).unwrap();
        assert!(nodes.is_empty(), "Database should remain empty after rollback");
    }

    // --- RUNTIME VALIDATION 3: Atomic Canonicalization ---
    #[test]
    fn test_runtime_atomic_canonicalization() {
        let test_db = TestStorage::new();
        let dispatcher: Arc<dyn RuntimeEventDispatcher> = Arc::new(InMemoryEventDispatcher::new(10));
        let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher));

        // 1. Lock the database exclusively on a separate connection to force failure during save or epoch increment
        let conn = test_db.storage().pool().get().unwrap();
        conn.execute("BEGIN EXCLUSIVE TRANSACTION", []).unwrap();

        // 2. Ingest a valid observation
        let obs = Observation {
            payload: b"Atomic Concept".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_c".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        // Canonicalization should fail because the database is locked exclusively by us, causing rollback
        let result = canonicalizer.canonicalize(obs);
        assert!(result.is_err(), "Canonicalization must fail due to exclusive database lock");

        // 3. Release the exclusive lock
        conn.execute("ROLLBACK", []).unwrap();
        drop(conn);

        // 4. Assert atomicity: config current_epoch was NOT updated and no node was saved
        let epoch_key = test_db.storage().run_transaction(|tx| tx.repositories().configs().get_key("current_epoch")).unwrap();
        assert!(epoch_key.is_none(), "Epoch key should not have been updated");

        let nodes = test_db.storage().run_transaction(|tx| tx.repositories().nodes().list_all()).unwrap();
        assert!(nodes.is_empty(), "Database must remain completely empty after aborted transaction");
    }

    // --- RUNTIME VALIDATION 4: Epoch Monotonicity Across Restarts ---
    #[test]
    fn test_runtime_epoch_monotonicity_across_restarts() {
        let test_db = TestStorage::new();
        let dispatcher: Arc<dyn RuntimeEventDispatcher> = Arc::new(InMemoryEventDispatcher::new(10));

        // Instance 1
        {
            let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher));
            let obs = Observation {
                payload: b"First Concept".to_vec(),
                media_type: "text/plain".to_string(),
                provenance: Provenance {
                    source_adapter: "adapter_d".to_string(),
                    timestamp: SystemTime::now(),
                    correlation_id: CorrelationId::new_v4(),
                },
            };
            let result = canonicalizer.canonicalize(obs).unwrap();
            assert_eq!(result.epoch.0, 1);
        }

        // Instance 2 (Reconstructed pointing to the same DB)
        {
            let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher));
            let obs = Observation {
                payload: b"Second Concept".to_vec(),
                media_type: "text/plain".to_string(),
                provenance: Provenance {
                    source_adapter: "adapter_e".to_string(),
                    timestamp: SystemTime::now(),
                    correlation_id: CorrelationId::new_v4(),
                },
            };
            let result = canonicalizer.canonicalize(obs).unwrap();
            assert_eq!(result.epoch.0, 2, "Epoch must increment monotonically to 2");
        }
    }

    // --- RUNTIME VALIDATION 5: Projection/Event Consistency ---
    #[test]
    fn test_runtime_projection_event_consistency() {
        let test_db = TestStorage::new();
        let dispatcher = Arc::new(InMemoryEventDispatcher::new(100));
        let mut rx = dispatcher.subscribe();
        let dispatcher_dyn: Arc<dyn RuntimeEventDispatcher> = dispatcher;

        let canonicalizer = SqliteCanonicalizer::new(test_db.storage().clone(), Arc::clone(&dispatcher_dyn));
        let epoch_mutex = Arc::new(Mutex::new(EpochId::initial()));
        let projection_manager = SqliteProjectionManager::new(test_db.storage().clone(), epoch_mutex, Arc::clone(&dispatcher_dyn));

        let corr_id = CorrelationId::new_v4();
        let obs = Observation {
            payload: b"Event-Consistent Concept".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_f".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: corr_id,
            },
        };

        // Ingest and assert event invalidation emission
        let result = canonicalizer.canonicalize(obs).unwrap();
        assert_eq!(result.epoch.0, 1);

        let mut received_invalidation = false;
        for _ in 0..10 {
            if let Ok(event) = rx.try_recv() {
                let event_any = event.as_any();
                if event_any.is::<ProjectionInstanceInvalidatedEvent>() {
                    let inv = event_any.downcast_ref::<ProjectionInstanceInvalidatedEvent>().unwrap();
                    assert_eq!(inv.projection_type, "MemoryListProjection");
                    assert_eq!(inv.epoch.0, 1);
                    assert_eq!(inv.correlation_id, corr_id);
                    received_invalidation = true;
                    break;
                }
            }
        }
        assert!(received_invalidation, "Should receive a ProjectionInstanceInvalidatedEvent");

        // Verify projection is rebuilt with the new node
        let query = MemoryListQuery { limit: 10 };
        let projector = SqliteProjector::new(test_db.storage().clone());
        let projection: MemoryListProjection = projection_manager.project(&projector, &query, corr_id);

        assert_eq!(projection.items.len(), 1);
        assert_eq!(projection.items[0].label, "Event-Consistent Concept");
    }

    // --- RUNTIME VALIDATION 6: Persistent Replay Determinism ---
    #[test]
    fn test_runtime_persistent_replay() {
        let obs_a = Observation {
            payload: b"Replay Concept A".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_g".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        let obs_b = Observation {
            payload: b"Replay Concept B".to_vec(),
            media_type: "text/plain".to_string(),
            provenance: Provenance {
                source_adapter: "adapter_h".to_string(),
                timestamp: SystemTime::now(),
                correlation_id: CorrelationId::new_v4(),
            },
        };

        // Target DB 1
        let db_1 = TestStorage::new();
        let disp_1 = Arc::new(InMemoryEventDispatcher::new(10));
        let canonicalizer_1 = SqliteCanonicalizer::new(db_1.storage().clone(), disp_1);

        // Target DB 2
        let db_2 = TestStorage::new();
        let disp_2 = Arc::new(InMemoryEventDispatcher::new(10));
        let canonicalizer_2 = SqliteCanonicalizer::new(db_2.storage().clone(), disp_2);

        // Run replays
        let res_1a = canonicalizer_1.canonicalize(obs_a.clone()).unwrap();
        let res_1b = canonicalizer_1.canonicalize(obs_b.clone()).unwrap();

        let res_2a = canonicalizer_2.canonicalize(obs_a.clone()).unwrap();
        let res_2b = canonicalizer_2.canonicalize(obs_b.clone()).unwrap();

        // 1. Verify semantic equality: Identical epoch sequences
        assert_eq!(res_1a.epoch, res_2a.epoch);
        assert_eq!(res_1b.epoch, res_2b.epoch);

        // 2. Verify semantic equality: Identical canonical entity IDs
        assert_eq!(res_1a.affected_entities, res_2a.affected_entities);
        assert_eq!(res_1b.affected_entities, res_2b.affected_entities);

        // 3. Verify semantic equality: Identical persisted nodes and fields
        let nodes_1 = db_1.storage().run_transaction(|tx| tx.repositories().nodes().list_all()).unwrap();
        let nodes_2 = db_2.storage().run_transaction(|tx| tx.repositories().nodes().list_all()).unwrap();
        assert_eq!(nodes_1.len(), nodes_2.len());
        for i in 0..nodes_1.len() {
            assert_eq!(nodes_1[i].id, nodes_2[i].id);
            assert_eq!(nodes_1[i].label, nodes_2[i].label);
            assert_eq!(nodes_1[i].node_type, nodes_2[i].node_type);
        }

        // 4. Verify semantic equality: Identical projection contents
        let manager_1 = SqliteProjectionManager::new(db_1.storage().clone(), Arc::new(Mutex::new(EpochId::initial())), Arc::new(InMemoryEventDispatcher::new(10)));
        let manager_2 = SqliteProjectionManager::new(db_2.storage().clone(), Arc::new(Mutex::new(EpochId::initial())), Arc::new(InMemoryEventDispatcher::new(10)));
        let projector_1 = SqliteProjector::new(db_1.storage().clone());
        let projector_2 = SqliteProjector::new(db_2.storage().clone());
        let query = MemoryListQuery { limit: 10 };

        let proj_1: MemoryListProjection = manager_1.project(&projector_1, &query, CorrelationId::new_v4());
        let proj_2: MemoryListProjection = manager_2.project(&projector_2, &query, CorrelationId::new_v4());

        assert_eq!(proj_1.items.len(), proj_2.items.len());
        for i in 0..proj_1.items.len() {
            assert_eq!(proj_1.items[i].id, proj_2.items[i].id);
            assert_eq!(proj_1.items[i].label, proj_2.items[i].label);
        }
    }
}
