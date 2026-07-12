import json

import pytest
from conftest import MockUdsServer

from brain_sdk import (
    BrainClient,
    BrainDaemonError,
    BrainProtocolError,
    IngestionEvent,
)


def test_protocol_newline_framing(temp_socket_path):
    server = MockUdsServer(temp_socket_path)
    recorded_requests = []

    def handler(conn):
        conn.settimeout(1.0)
        data = conn.recv(1024 * 10)
        if data:
            recorded_requests.append(data)
            resp = {"status": "ok", "message": '{"sequence":1,"event_id":"abc"}'}
            conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with BrainClient(socket_path=temp_socket_path) as client:
            client.send(IngestionEvent.text("hello"))
        
        assert len(recorded_requests) == 1
        raw_req = recorded_requests[0].decode("utf-8")
        assert raw_req.endswith("\n")
        assert raw_req.count("\n") == 1
    finally:
        server.stop()

def test_protocol_malformed_outer_json(temp_socket_path):
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        conn.sendall(b"this is not json\n")
        conn.close()

    server.start(handler)

    try:
        with pytest.raises(BrainProtocolError) as excinfo:
            with BrainClient(socket_path=temp_socket_path) as client:
                client.send(IngestionEvent.text("hello"))
        assert "Malformed outer JSON response" in str(excinfo.value)
    finally:
        server.stop()

def test_protocol_malformed_inner_json(temp_socket_path):
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        resp = {"status": "ok", "message": "not inner json"}
        conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with pytest.raises(BrainProtocolError) as excinfo:
            with BrainClient(socket_path=temp_socket_path) as client:
                client.send(IngestionEvent.text("hello"))
        assert "Malformed inner IngestAck" in str(excinfo.value)
    finally:
        server.stop()

def test_protocol_unknown_response_fields(temp_socket_path):
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        ack = {"sequence": 5, "event_id": "uuid-123", "ignored_field": "val"}
        resp = {"status": "ok", "message": json.dumps(ack), "another_ignored_field": 42}
        conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with BrainClient(socket_path=temp_socket_path) as client:
            ack = client.send(IngestionEvent.text("hello"))
            assert ack.sequence == 5
            assert ack.event_id == "uuid-123"
    finally:
        server.stop()

def test_protocol_missing_required_fields(temp_socket_path):
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        ack = {"sequence": 5}
        resp = {"status": "ok", "message": json.dumps(ack)}
        conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with pytest.raises(BrainProtocolError) as excinfo:
            with BrainClient(socket_path=temp_socket_path) as client:
                client.send(IngestionEvent.text("hello"))
        assert "Missing required fields in IngestAck" in str(excinfo.value)
    finally:
        server.stop()

def test_protocol_daemon_error_response(temp_socket_path):
    server = MockUdsServer(temp_socket_path)

    def handler(conn):
        conn.settimeout(1.0)
        resp = {"status": "error", "message": "Deduplication conflict"}
        conn.sendall((json.dumps(resp) + "\n").encode("utf-8"))
        conn.close()

    server.start(handler)

    try:
        with pytest.raises(BrainDaemonError) as excinfo:
            with BrainClient(socket_path=temp_socket_path) as client:
                client.send(IngestionEvent.text("hello"))
        assert "Deduplication conflict" in str(excinfo.value)
    finally:
        server.stop()
