use brain_storage::{SqliteEventLog, TestStorage};
use uuid::Uuid;

#[test]
fn test_sequential_write_and_read() {
    let test_storage = TestStorage::new();
    let event_log = SqliteEventLog::new(test_storage.store().pool().clone());

    // 1. Write 3 events
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();
    let corr = Uuid::new_v4();

    let seq1 = event_log
        .append(id1, corr, 100, "1.0", "src1", "system", "{}")
        .unwrap();
    let seq2 = event_log
        .append(id2, corr, 200, "1.0", "src2", "system", "{}")
        .unwrap();
    let seq3 = event_log
        .append(id3, corr, 300, "1.0", "src3", "system", "{}")
        .unwrap();

    assert_eq!(seq1, 1);
    assert_eq!(seq2, 2);
    assert_eq!(seq3, 3);

    assert_eq!(event_log.latest_sequence().unwrap(), 3);

    // 2. Read them back
    let results = event_log.read_from(1, 10).unwrap();
    assert_eq!(results.len(), 3);

    assert_eq!(results[0].sequence, 1);
    assert_eq!(results[0].source, "src1");
    assert_eq!(results[0].event_id, id1);

    assert_eq!(results[1].sequence, 2);
    assert_eq!(results[1].source, "src2");
    assert_eq!(results[1].event_id, id2);

    assert_eq!(results[2].sequence, 3);
    assert_eq!(results[2].source, "src3");
    assert_eq!(results[2].event_id, id3);
}

#[test]
fn test_empty_log_safety() {
    let test_storage = TestStorage::new();
    let event_log = SqliteEventLog::new(test_storage.store().pool().clone());

    let results = event_log.read_from(1, 10).unwrap();
    assert!(results.is_empty());

    assert_eq!(event_log.latest_sequence().unwrap(), 0);
}

#[test]
fn test_strict_sequence_ordering() {
    let test_storage = TestStorage::new();
    let event_log = SqliteEventLog::new(test_storage.store().pool().clone());

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();
    let corr = Uuid::new_v4();

    // Append in sequence order: 1, 2, 3
    event_log
        .append(id1, corr, 100, "1.0", "src1", "system", "{}")
        .unwrap();
    event_log
        .append(id2, corr, 50, "1.0", "src2", "system", "{}")
        .unwrap();
    event_log
        .append(id3, corr, 10, "1.0", "src3", "system", "{}")
        .unwrap();

    // Verify read order sorts strictly by auto-increment sequence, not timestamp
    let results = event_log.read_from(1, 10).unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].sequence, 1);
    assert_eq!(results[0].timestamp_ms, 100);

    assert_eq!(results[1].sequence, 2);
    assert_eq!(results[1].timestamp_ms, 50);

    assert_eq!(results[2].sequence, 3);
    assert_eq!(results[2].timestamp_ms, 10);
}

#[test]
fn test_pagination_boundaries() {
    let test_storage = TestStorage::new();
    let event_log = SqliteEventLog::new(test_storage.store().pool().clone());

    let corr = Uuid::new_v4();

    // Append 25 events
    for i in 1..=25 {
        event_log
            .append(Uuid::new_v4(), corr, 1000 + i, "1.0", "src", "system", "{}")
            .unwrap();
    }

    // Page 1: sequence 1-10
    let page1 = event_log.read_from(1, 10).unwrap();
    assert_eq!(page1.len(), 10);
    assert_eq!(page1.first().unwrap().sequence, 1);
    assert_eq!(page1.last().unwrap().sequence, 10);

    // Page 2: sequence 11-20
    let page2 = event_log.read_from(11, 10).unwrap();
    assert_eq!(page2.len(), 10);
    assert_eq!(page2.first().unwrap().sequence, 11);
    assert_eq!(page2.last().unwrap().sequence, 20);

    // Page 3: sequence 21-30 (only 5 left)
    let page3 = event_log.read_from(21, 10).unwrap();
    assert_eq!(page3.len(), 5);
    assert_eq!(page3.first().unwrap().sequence, 21);
    assert_eq!(page3.last().unwrap().sequence, 25);
}

#[test]
fn test_ingestion_event_log_repository() {
    use brain_integrations::{EventIdentity, IngestionEnvelope, IngestionEvent};
    use brain_storage::EventLogRepository;

    let test_storage = TestStorage::new();
    let storage = test_storage.store();

    // 1. Create a test IngestionEnvelope
    let event_id1 = brain_domain::EventId(uuid::Uuid::new_v4());
    let envelope1 = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: event_id1.clone(),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("workspace-1"),
            client_id: brain_domain::ClientId::new("client-1"),
            adapter_id: brain_domain::AdapterId::new("adapter-1"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: Some(brain_domain::ConversationId(ulid::Ulid::new())),
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "user".to_string(),
            content: "Ping observation 1".to_string(),
            metadata: std::collections::BTreeMap::new(),
        },
    };

    // 2. Insert event
    let seq1 = storage.insert_event(&envelope1).unwrap();
    assert_eq!(seq1, 1);

    // 3. Check duplicate (same event_id should return existing sequence, not insert a new row)
    let seq_dup = storage.insert_event(&envelope1).unwrap();
    assert_eq!(seq_dup, 1);

    // Check duplicate checker directly
    let is_dup = storage.is_duplicate_event(&event_id1).unwrap();
    assert!(is_dup);

    let event_id2 = brain_domain::EventId(uuid::Uuid::new_v4());
    let is_dup2 = storage.is_duplicate_event(&event_id2).unwrap();
    assert!(!is_dup2);

    // 4. Insert another event
    let envelope2 = IngestionEnvelope {
        event_model_version: "1.0".to_string(),
        identity: EventIdentity {
            event_id: event_id2.clone(),
            parent_event_id: None,
            workspace_id: brain_domain::WorkspaceId::new("workspace-1"),
            client_id: brain_domain::ClientId::new("client-1"),
            adapter_id: brain_domain::AdapterId::new("adapter-1"),
            session_id: "01H7X1F8Z9Y000000000000000".parse().unwrap(),
            conversation_id: None,
            timestamp: chrono::Utc::now(),
        },
        event: IngestionEvent::Message {
            role: "assistant".to_string(),
            content: "Response observation 2".to_string(),
            metadata: std::collections::BTreeMap::new(),
        },
    };
    let seq2 = storage.insert_event(&envelope2).unwrap();
    assert_eq!(seq2, 2);

    // 5. Replay events
    let events = storage.get_events_after(0).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].identity.event_id, event_id1);
    assert_eq!(events[1].identity.event_id, event_id2);

    // Replay after sequence 1
    let events_after = storage.get_events_after(1).unwrap();
    assert_eq!(events_after.len(), 1);
    assert_eq!(events_after[0].identity.event_id, event_id2);
}
