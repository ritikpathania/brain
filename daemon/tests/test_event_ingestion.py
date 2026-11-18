"""Versioned ingestion coverage against the hermetic daemon fixture."""

import json
import uuid
from typing import cast


def generate_envelope(event_id: str, content: str = "Hello Brain") -> dict[str, object]:
    return {
        "event_model_version": "1.0",
        "identity": {
            "event_id": event_id,
            "parent_event_id": None,
            "workspace_id": "python-uds-ci",
            "client_id": "python-uds-ci",
            "adapter_id": "python-uds-ci",
            "session_id": "01H7X1F8Z9Y000000000000000",
            "conversation_id": None,
            "timestamp": "2026-06-29T15:30:00Z",
        },
        "event": {
            "event_type": "text",
            "content": content,
            "metadata": {},
        },
    }


def test_versioned_ingest_is_processed_by_the_isolated_daemon(uds_runtime):
    envelope = generate_envelope(str(uuid.uuid4()), "Python UDS versioned ingestion")

    response = uds_runtime.call("v1/ingest", json.dumps(envelope))

    assert response["version"] == "1.0"
    assert response["type"] == "Response"
    assert response["status"] == "success"
    body = json.loads(response["body"])
    identity = cast(dict[str, object], envelope["identity"])
    assert body["processed"] is True
    assert body["event_id"] == identity["event_id"]
