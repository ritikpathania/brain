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


def clean_stop_daemon(daemon_exe):
    import signal
    import subprocess

    pid_path = os.path.expanduser("~/.brain/daemon.pid")
    socket_path = os.path.expanduser("~/.brain/daemon.sock")
    pid = None
    if os.path.exists(pid_path):
        try:
            with open(pid_path) as f:
                pid = int(f.read().strip())
        except Exception:
            pass

    subprocess.run([daemon_exe, "daemon", "stop"], capture_output=True)
    time.sleep(0.3)

    if pid:
        for _ in range(20):
            try:
                os.kill(pid, 0)
            except OSError:
                break
            try:
                os.kill(pid, signal.SIGKILL)
            except Exception:
                pass
            time.sleep(0.1)

    if os.path.exists(pid_path):
        try:
            os.remove(pid_path)
        except Exception:
            pass
    if os.path.exists(socket_path):
        try:
            os.remove(socket_path)
        except Exception:
            pass

    # Clean any other lingering brain-daemon processes to avoid socket lock conflicts
    subprocess.run(["pkill", "-9", "brain-daemon"], capture_output=True)
    time.sleep(0.3)


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
    assert "success" in ingest_responses[0]["message"]

    # 2. Query with retries for asynchronous projection visibility
    query_responses = []
    reconstructed_content = ""

    for _ in range(15):
        time.sleep(0.3)
        query_responses = run_uds_query("query", "rust")
        reconstructed_content = "".join(
            r.get("content", "") for r in query_responses if r.get("type") == "stream_chunk"
        )
        if "Found" in reconstructed_content:
            break

    assert len(query_responses) > 0

    # Verify the stream lifecycle events
    event_types = [r.get("type") for r in query_responses]
    assert "stream_start" in event_types
    assert "stream_end" in event_types

    # Accumulate chunks and validate sequence monotonicity
    expected_seq = None

    for r in query_responses:
        # Check that events requiring sequence numbers have monotonic increments
        if r.get("type") in STREAM_EVENTS:
            seq = r.get("sequence")
            if expected_seq is None:
                expected_seq = seq
            assert seq == expected_seq
            expected_seq += 1

    assert "Found" in reconstructed_content
    assert "result" in reconstructed_content


def test_top_result_has_high_confidence():
    """Query top result must display 'High confidence'."""
    ingest_payload = "User ritikpathania is testing rust confidence ranking."
    ingest_res = run_uds_query("ingest", ingest_payload)
    assert len(ingest_res) > 0
    assert ingest_res[0].get("status") == "ok"

    # Query with retries for asynchronous projection updates
    for _ in range(15):
        time.sleep(0.3)
        responses = run_uds_query("query", "testing")
        chunks = [
            r.get("content", "") for r in responses if r.get("type") == "stream_chunk"
        ]
        result_chunks = [c for c in chunks if "confidence" in c]
        if result_chunks:
            assert "High confidence" in result_chunks[0], (
                f"Expected 'High confidence' in top result, got: {result_chunks[0]!r}"
            )
            return

    pytest.fail("No confidence chunk received in query response")
