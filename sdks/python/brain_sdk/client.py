import datetime
import json
import os
import uuid
from pathlib import Path

from brain_sdk.errors import (
    BrainConnectionError,
    BrainDaemonError,
    BrainProtocolError,
)
from brain_sdk.models import (
    EventIdentity,
    IngestAck,
    IngestionEnvelope,
    JSONDict,
)
from brain_sdk.transport import UdsTransport


def default_socket_path() -> Path:
    """Return the default socket path matching (~/.brain/daemon.sock)."""
    home = os.environ.get("HOME")
    if home:
        return Path(home) / ".brain" / "daemon.sock"
    return Path("/tmp/brain-daemon.sock")


class BrainClient:
    """Synchronous reference client for the Brain Ingestion Daemon."""

    def __init__(
        self,
        socket_path: str | Path | None = None,
        workspace_id: str = "default",
        client_id: str = "python-sdk",
        adapter_id: str = "python-default",
        session_id: str | None = None,
        timeout: float = 5.0,
    ):
        path = Path(socket_path) if socket_path else default_socket_path()
        self.workspace_id = workspace_id
        self.client_id = client_id
        self.adapter_id = adapter_id
        self.session_id = session_id or str(uuid.uuid4())
        self.timeout = timeout
        self.transport = UdsTransport(path, timeout)
        self._connected = False

    def __enter__(self) -> "BrainClient":
        self.connect()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()

    def connect(self):
        """Establish the underlying socket connection."""
        if not self._connected:
            self.transport.connect()
            self._connected = True

    def send(self, event: JSONDict) -> IngestAck:
        """Construct the envelope, stream it over UDS, and parse the return ACK."""
        if not self._connected:
            raise BrainConnectionError(
                "Client is not connected. Use connect() or the context manager."
            )

        # 1. Generate identity & envelope DTOs
        identity = EventIdentity(
            event_id=str(uuid.uuid4()),
            workspace_id=self.workspace_id,
            client_id=self.client_id,
            adapter_id=self.adapter_id,
            session_id=self.session_id,
            timestamp=datetime.datetime.now(datetime.UTC).isoformat(),
        )
        envelope = IngestionEnvelope(
            event_model_version="1.0", identity=identity, event=event
        )

        # 2. Form the outer UDS frame
        request = {"action": "ingest_event", "payload": json.dumps(envelope.to_dict())}
        wire_payload = json.dumps(request)

        # 3. Transmit
        self.transport.write_line(wire_payload)

        # 4. Await response & parse DTO
        response_str = self.transport.read_line()
        try:
            resp_json = json.loads(response_str)
        except json.JSONDecodeError as e:
            raise BrainProtocolError(
                f"Malformed outer JSON response from daemon: {e}"
            ) from e

        status = resp_json.get("status")
        if status not in ("ok", "success"):
            # Check for error description in body/message
            err_msg = (
                resp_json.get("body")
                or resp_json.get("message")
                or "Unknown daemon error"
            )
            raise BrainDaemonError(f"Daemon returned failure: {err_msg}")

        body_str = resp_json.get("body") or resp_json.get("message")
        if body_str is None:
            raise BrainProtocolError("Missing body/message payload in daemon response")

        try:
            ack_json = json.loads(body_str)
        except json.JSONDecodeError as e:
            raise BrainProtocolError(
                f"Malformed inner IngestAck JSON response: {e}"
            ) from e

        sequence = ack_json.get("sequence")
        event_id = ack_json.get("event_id")

        if sequence is None or event_id is None:
            raise BrainProtocolError(
                f"Missing required fields in IngestAck: "
                f"sequence={sequence}, event_id={event_id}"
            )

        try:
            return IngestAck(sequence=int(sequence), event_id=str(event_id))
        except (ValueError, TypeError) as e:
            raise BrainProtocolError(f"Invalid type in IngestAck fields: {e}") from e

    def close(self):
        """Teardown transport and close the connection."""
        self.transport.close()
        self._connected = False
