#[cfg(test)]
mod tests {
    use brain_domain::{GraphProvenance, Node, NodeId, NodeKind, ProvenanceSource};
    use brain_storage::SqliteStorage;
    use std::time::SystemTime;

    fn current_time_secs() -> u64 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    // --- RELIABILITY TEST 1: Interrupted Transaction Recovery ---
    #[test]
    fn test_reliability_interrupted_transaction_recovery() {
        // Create a unique DB path to manage manually
        let mut temp_db = std::env::temp_dir();
        let db_uuid = uuid::Uuid::new_v4();
        temp_db.push(format!("brain_reliability_test_1_{}.db", db_uuid));
        let db_path = temp_db.to_str().unwrap().to_string();

        // 1. Open storage instance
        let storage = SqliteStorage::new(&db_path, 1, true).unwrap();

        let node_id = NodeId::new();
        let mut node = Node::new(node_id, "Aborted Node".to_string(), NodeKind::Concept);
        node.provenance = GraphProvenance {
            source_conversation: None,
            source_message: None,
            extracted_at: current_time_secs(),
            extractor_version: "v1.0.0".to_string(),
            confidence: 1.0,
            text_span: None,
            source: ProvenanceSource::Imported,
        };

        // 2. Execute transaction and simulate interrupt by returning a Storage error (aborting transaction)
        let result: Result<(), _> = storage.run_transaction(|tx| {
            tx.repositories().nodes().save(&node)?;

            Err(brain_core::errors::BrainError::Storage {
                message: "Simulated Interrupted Transaction".to_string(),
                source: None,
            })
        });

        assert!(result.is_err(), "Transaction must fail and abort");

        // 3. Open a second storage instance pointing to the same DB file
        let new_storage = SqliteStorage::new(&db_path, 1, true).unwrap();
        let nodes = new_storage
            .run_transaction(|tx| tx.repositories().nodes().list_all())
            .unwrap();
        assert!(
            nodes.is_empty(),
            "Uncommitted transaction modifications must be completely rolled back"
        );

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path));
        let _ = std::fs::remove_file(format!("{}-shm", db_path));
    }

    // --- RELIABILITY TEST 2: Forced Restart Consistency ---
    #[test]
    fn test_reliability_forced_restart_consistency() {
        // Create a unique DB path to manage manually
        let mut temp_db = std::env::temp_dir();
        let db_uuid = uuid::Uuid::new_v4();
        temp_db.push(format!("brain_reliability_test_2_{}.db", db_uuid));
        let db_path = temp_db.to_str().unwrap().to_string();

        // 1. Initial State: Save a committed node
        {
            let storage = SqliteStorage::new(&db_path, 1, true).unwrap();
            let node_id = NodeId::new();
            let mut node = Node::new(node_id, "Committed Node".to_string(), NodeKind::Concept);
            node.provenance = GraphProvenance {
                source_conversation: None,
                source_message: None,
                extracted_at: current_time_secs(),
                extractor_version: "v1.0.0".to_string(),
                confidence: 1.0,
                text_span: None,
                source: ProvenanceSource::Imported,
            };
            storage
                .run_transaction(|tx| tx.repositories().nodes().save(&node))
                .unwrap();
        }

        // 2. Mock sudden shutdown/abort mid-transaction
        {
            let storage = SqliteStorage::new(&db_path, 1, true).unwrap();
            let node_id = NodeId::new();
            let mut crashed_node = Node::new(
                node_id,
                "Crashed mid-tx Node".to_string(),
                NodeKind::Concept,
            );
            crashed_node.provenance = GraphProvenance {
                source_conversation: None,
                source_message: None,
                extracted_at: current_time_secs(),
                extractor_version: "v1.0.0".to_string(),
                confidence: 1.0,
                text_span: None,
                source: ProvenanceSource::Imported,
            };

            let _: Result<(), _> = storage.run_transaction(|tx| {
                tx.repositories().nodes().save(&crashed_node)?;

                Err(brain_core::errors::BrainError::Storage {
                    message: "Sudden crash mid-transaction".to_string(),
                    source: None,
                })
            });
        }

        // 3. Restart: Reopen storage and assert committed state remains valid while crashed state is missing
        let restarted_storage = SqliteStorage::new(&db_path, 1, true).unwrap();
        let nodes = restarted_storage
            .run_transaction(|tx| tx.repositories().nodes().list_all())
            .unwrap();

        assert_eq!(
            nodes.len(),
            1,
            "Only the committed node should survive the crash"
        );
        assert_eq!(nodes[0].label, "Committed Node");

        // Cleanup
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path));
        let _ = std::fs::remove_file(format!("{}-shm", db_path));
    }
}
