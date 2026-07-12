import time

import pytest
from conftest import MockUdsServer

from brain_sdk import (
    BrainClient,
    BrainConnectionError,
    BrainTimeoutError,
    IngestionEvent,
)


def test_successful_ingestion(temp_socket_path):
    server = MockUdsServer(temp_socket_path)
    
    def handler(conn):
        conn.settimeout(1.0)
        f = conn.makefile("r", encoding="utf-8")
        line = f.readline()
        if line:
            import json
            req = json.loads(line)
            assert req["action"] == "ingest_event"
            payload = json.loads(req["payload"])
            event_id = payload["identity"]["event_id"]
            
            ack = {"sequence": 999, "event_id": event_id}
            resp = {"status": "ok", "message": json.dumps(ack)}
            conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with BrainClient(socket_path=temp_socket_path) as client:
            event = IngestionEvent.message(role="user", content="Hi")
            ack = client.send(event)
            assert ack.sequence == 999
            assert len(ack.event_id) > 0
    finally:
        server.stop()

def test_connection_refused(temp_socket_path):
    with pytest.raises(BrainConnectionError):
        with BrainClient(socket_path=temp_socket_path):
            # send raises error because connect() is called on entering __enter__
            pass

def test_timeout_on_read(temp_socket_path):
    server = MockUdsServer(temp_socket_path)
    
    def handler(conn):
        time.sleep(0.3)
        conn.close()

    server.start(handler)

    try:
        with pytest.raises(BrainTimeoutError):
            with BrainClient(socket_path=temp_socket_path, timeout=0.05) as client:
                client.send(IngestionEvent.text("hello"))
    finally:
        server.stop()
