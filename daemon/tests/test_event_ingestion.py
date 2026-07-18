import json
import uuid

from tests.test_uds_ipc import run_uds_query


def generate_envelope(event_id, content="Hello Brain"):
    return {
        "event_model_version": "1.0",
        "identity": {
            "event_id": event_id,
            "parent_event_id": "b3b7541f-8239-44d4-95e2-b91c0683072b",
            "workspace_id": "proj-123",
            "client_id": "cursor-1.0",
            "adapter_id": "vscode-ext",
            "session_id": "01H7X1F8Z9Y000000000000000",
            "conversation_id": "01H7X1F8Z9Y000000000000001",
            "timestamp": "2026-06-29T15:30:00Z",
        },
        "event": {
            "event_type": "message",
            "role": "user",
            "content": content,
            "metadata": {"provider.model": "claude-3-5"},
        },
    }


def test_end_to_end_event_ingestion_and_deduplication():
    # 1. Generate unique event ID
    event_id = str(uuid.uuid4())
    envelope = generate_envelope(event_id, "Hello Brain from End-to-End Test")
    payload = json.dumps(envelope)

    # 2. Ingest event
    responses = run_uds_query("ingest_event", payload)
    assert len(responses) > 0
    assert responses[0]["status"] == "ok"

    ack_data = json.loads(responses[0]["message"])
    assert "sequence" in ack_data
    assert ack_data["event_id"] == event_id
    seq1 = ack_data["sequence"]
    assert seq1 >= 1

    # 3. Ingest the same event again (deduplication check)
    responses_dup = run_uds_query("ingest_event", payload)
    assert len(responses_dup) > 0
    assert responses_dup[0]["status"] == "ok"

    ack_data_dup = json.loads(responses_dup[0]["message"])
    assert ack_data_dup["sequence"] == seq1
    assert ack_data_dup["event_id"] == event_id


def test_end_to_end_event_replay():
    # 1. Ingest a new event
    event_id = str(uuid.uuid4())
    content = f"Replay check {event_id}"
    envelope = generate_envelope(event_id, content)
    payload = json.dumps(envelope)

    ingest_responses = run_uds_query("ingest_event", payload)
    ack_data = json.loads(ingest_responses[0]["message"])
    seq = ack_data["sequence"]

    # 2. Replay from seq - 1
    replay_responses = run_uds_query("replay", str(seq - 1))
    assert len(replay_responses) > 0
    assert replay_responses[0]["status"] == "ok"

    events = json.loads(replay_responses[0]["message"])
    assert isinstance(events, list)
    assert len(events) >= 1

    # Find our ingested event in the replayed list
    matched_event = None
    for ev in events:
        if ev["identity"]["event_id"] == event_id:
            matched_event = ev
            break

    assert matched_event is not None
    assert matched_event["event_model_version"] == "1.0"
    assert matched_event["event"]["content"] == content
