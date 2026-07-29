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
    assert seq1 > 0
