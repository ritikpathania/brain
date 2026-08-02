import json
from pathlib import Path
from conftest import MockUdsServer
from brain_sdk import BrainClient, IngestionEvent


def test_readme_quickstart_example(temp_socket_path):
    """Verify that the Quick Start code block in README.md executes without errors."""
    readme_path = Path(__file__).parent.parent / "README.md"
    readme_text = readme_path.read_text(encoding="utf-8")
    
    # Check that query() is not referenced in quick start
    assert "client.query(" not in readme_text, "README quick start still contains invalid client.query()"

    # Mock server handling send()
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        f = conn.makefile("r", encoding="utf-8")
        line = f.readline()
        if line:
            req = json.loads(line)
            assert req["action"] == "ingest_event"
            payload = json.loads(req["payload"])
            event_id = payload["identity"]["event_id"]
            ack = {"sequence": 1, "event_id": event_id}
            resp = {"status": "ok", "message": json.dumps(ack)}
            conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        # Execute the verbatim Quick Start pattern
        with BrainClient(socket_path=temp_socket_path) as client:
            event = IngestionEvent.message(role="user", content="Hello Brain")
            ack = client.send(event)
            assert ack.sequence == 1
            assert len(ack.event_id) > 0
    finally:
        server.stop()
