import json
import os
import socket
import time

import pytest
from jsonschema import Draft7Validator, RefResolver

# Set of events in the streaming protocol that require sequence numbers
STREAM_EVENTS = {
    "stream_progress",
    "stream_chunk",
    "stream_end",
}

SCHEMA_PATH = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "../../protocol/uds_ipc.schema.json")
)

# Initialize and compile schema validators
with open(SCHEMA_PATH) as f:
    _schema = json.load(f)
_resolver = RefResolver.from_schema(_schema)
RESPONSE_VALIDATOR = Draft7Validator(_schema["$defs"]["Response"], resolver=_resolver)
STREAM_VALIDATOR = Draft7Validator(_schema["$defs"]["StreamEvent"], resolver=_resolver)


def validate_message(msg):
    """
    Validates a received message against the JSON Schema wire protocol contract.
    Raises jsonschema.ValidationError on mismatch.
    """
    if not isinstance(msg, dict):
        return
    t = msg.get("type", "")
    if isinstance(t, str) and t.startswith("stream_"):
        STREAM_VALIDATOR.validate(msg)
    else:
        RESPONSE_VALIDATOR.validate(msg)


def run_uds_query(action, payload_text):
    socket_path = os.path.expanduser("~/.brain/daemon.sock")

    # Wait up to 3 seconds for the socket file to exist AND be connectable/ready
    ready = False
    for _ in range(30):
        if os.path.exists(socket_path):
            try:
                # Try to establish a short-lived connection to verify readiness
                test_socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
                test_socket.settimeout(0.1)
                test_socket.connect(socket_path)
                test_socket.close()
                ready = True
                break
            except (OSError, ConnectionRefusedError):
                pass
        time.sleep(0.1)

    if not ready:
        pytest.skip(
            f"Daemon UDS socket not ready at {socket_path}. Is the daemon running?"
        )

    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(socket_path)

    request = {"action": action, "payload": payload_text}
    s.sendall((json.dumps(request) + "\n").encode())

    buffer = b""
    responses = []

    # Read response
    while True:
        chunk = s.recv(1024)
        if not chunk:
            break
        buffer += chunk

        while b"\n" in buffer:
            line, buffer = buffer.split(b"\n", 1)
            resp = json.loads(line.decode())

            # Validate wire payload conformance before checking logic
            validate_message(resp)

            responses.append(resp)

            # Stop if stream terminal event or legacy response terminal status
            if isinstance(resp, dict):
                if resp.get("type") in ["stream_end", "stream_cancelled"]:
                    s.close()
                    return responses
                if (
                    resp.get("status") in ["ok", "error"]
                    and resp.get("type") != "stream_progress"
                ):
                    s.close()
                    return responses
    s.close()
    return responses


def test_uds_ingest_and_query_stream():
    # 1. Ingest a sentence
    ingest_payload = "User ritikpathania is testing rust and uds socket connections."
    ingest_responses = run_uds_query("ingest", ingest_payload)

    assert len(ingest_responses) > 0
    assert ingest_responses[0]["status"] == "ok"
    assert "Ingested node" in ingest_responses[0]["message"]

    # 2. Query immediately (hits STM Cache)
    query_responses = run_uds_query("query", "rust")

    assert len(query_responses) > 0

    # Verify the stream lifecycle events
    event_types = [r.get("type") for r in query_responses]
    assert "stream_start" in event_types
    assert "stream_end" in event_types

    # Accumulate chunks and validate sequence monotonicity
    reconstructed_content = ""
    expected_seq = None

    for r in query_responses:
        # Check that events requiring sequence numbers have monotonic increments
        if r.get("type") in STREAM_EVENTS:
            seq = r.get("sequence")
            if expected_seq is None:
                expected_seq = seq
            assert seq == expected_seq
            expected_seq += 1

        if r.get("type") == "stream_chunk":
            reconstructed_content += r.get("content", "")

    assert "Found" in reconstructed_content
    assert "result" in reconstructed_content
    assert "rust" in reconstructed_content.lower()


def test_uds_compatibility_parity():
    # Resolve daemon executable path relative to the test runner's directory
    daemon_exe = (
        "../brain-daemon" if os.path.exists("../brain-daemon") else "./brain-daemon"
    )

    # Restart the daemon with BRAIN_DEBUG=1 to expose raw identifiers
    import subprocess

    subprocess.run([daemon_exe, "daemon", "stop"], capture_output=True)
    env = os.environ.copy()
    env["BRAIN_DEBUG"] = "1"
    subprocess.run([daemon_exe, "daemon", "start"], env=env, capture_output=True)
    time.sleep(1)

    try:
        # 1. Ingest a sentence with a unique keyword to isolate results
        unique_keyword = "antigravityparitytest"
        ingest_payload = (
            f"The unique target for {unique_keyword} has been successfully validated."
        )
        ingest_responses = run_uds_query("ingest", ingest_payload)

        assert len(ingest_responses) > 0
        assert ingest_responses[0]["status"] in ("ok", "success")
        msg = ingest_responses[0]["message"]
        assert "Ingested node" in msg

        # Parse node ID (UUID) from success message
        # Format: "Ingested node '{id}' (Epoch {epoch}) successfully"
        import re

        match = re.search(r"Ingested node '([^']+)' \(Epoch (\d+)\) successfully", msg)
        assert match is not None, f"Failed to match UUID pattern in message: {msg}"
        node_id = match.group(1)

        # 2. Query immediately (hits STM Cache compatibility layer)
        query_responses = run_uds_query("query", unique_keyword)
        assert len(query_responses) > 0

        reconstructed_content = ""
        for r in query_responses:
            if r.get("type") == "stream_chunk":
                reconstructed_content += r.get("content", "")

        # Assert compatibility matching
        assert "Found" in reconstructed_content
        # The output rendered contains details of matching nodes.
        # Check that the UUID and content are present in the response
        assert node_id in reconstructed_content
        assert unique_keyword in reconstructed_content
    finally:
        # Restore daemon to standard state
        subprocess.run([daemon_exe, "daemon", "stop"], capture_output=True)
        subprocess.run([daemon_exe, "daemon", "start"], capture_output=True)
        time.sleep(1)
