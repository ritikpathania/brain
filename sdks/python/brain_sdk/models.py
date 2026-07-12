"""Data models for the Brain SDK."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

JSONValue = (
    str | int | float | bool | None |
    list["JSONValue"] |
    dict[str, "JSONValue"]
)

JSONDict = dict[str, JSONValue]

@dataclass(frozen=True)
class EventIdentity:
    """Stable metadata describing an event's source and session identity."""
    event_id: str
    workspace_id: str
    client_id: str
    adapter_id: str
    session_id: str
    timestamp: str
    parent_event_id: str | None = None
    conversation_id: str | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert the identity to a serializable dictionary."""
        return {
            "adapter_id": self.adapter_id,
            "client_id": self.client_id,
            "conversation_id": self.conversation_id,
            "event_id": self.event_id,
            "parent_event_id": self.parent_event_id,
            "session_id": self.session_id,
            "timestamp": self.timestamp,
            "workspace_id": self.workspace_id,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> EventIdentity:
        """Reconstruct from dictionary."""
        return cls(
            event_id=data["event_id"],
            workspace_id=data["workspace_id"],
            client_id=data["client_id"],
            adapter_id=data["adapter_id"],
            session_id=data["session_id"],
            timestamp=data["timestamp"],
            parent_event_id=data.get("parent_event_id"),
            conversation_id=data.get("conversation_id"),
        )

@dataclass(frozen=True)
class IngestionEnvelope:
    """Standard payload structure wrapping version, source, and event."""
    event_model_version: str
    identity: EventIdentity
    event: JSONDict

    def to_dict(self) -> dict[str, Any]:
        """Convert the envelope to a serializable dictionary."""
        return {
            "event": self.event,
            "event_model_version": self.event_model_version,
            "identity": self.identity.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> IngestionEnvelope:
        """Reconstruct from dictionary."""
        return cls(
            event_model_version=data["event_model_version"],
            identity=EventIdentity.from_dict(data["identity"]),
            event=data["event"],
        )

@dataclass(frozen=True)
class IngestAck:
    """Response returned from the daemon indicating successful event persistence."""
    sequence: int
    event_id: str

class IngestionEvent:
    """Helpers to construct canonical event dictionaries."""

    @staticmethod
    def message(role: str, content: str, metadata: JSONDict | None = None) -> JSONDict:
        """Construct a message event dictionary."""
        return {
            "event_type": "message",
            "role": role,
            "content": content,
            "metadata": metadata or {},
        }

    @staticmethod
    def text(content: str, metadata: JSONDict | None = None) -> JSONDict:
        """Construct a fallback text event dictionary."""
        return {
            "event_type": "text",
            "content": content,
            "metadata": metadata or {},
        }

    @staticmethod
    def file_edit(
        path: str, diff: str | None = None, metadata: JSONDict | None = None
    ) -> JSONDict:
        """Construct a file edit workspace event."""
        e: JSONDict = {
            "event_type": "file_edit",
            "path": path,
            "metadata": metadata or {},
        }
        if diff is not None:
            e["diff"] = diff
        return e

    @staticmethod
    def tool_call(
        tool_name: str,
        call_id: str,
        arguments: JSONValue,
        metadata: JSONDict | None = None,
    ) -> JSONDict:
        """Construct a tool invocation event."""
        return {
            "event_type": "tool_call",
            "tool_name": tool_name,
            "call_id": call_id,
            "arguments": arguments,
            "metadata": metadata or {},
        }

    @staticmethod
    def tool_result(
        call_id: str,
        is_error: bool,
        output: str,
        metadata: JSONDict | None = None,
    ) -> JSONDict:
        """Construct a tool result event."""
        return {
            "event_type": "tool_result",
            "call_id": call_id,
            "is_error": is_error,
            "output": output,
            "metadata": metadata or {},
        }

    @staticmethod
    def terminal_command(
        command: str,
        exit_code: int | None = None,
        stdout_summary: str | None = None,
        metadata: JSONDict | None = None,
    ) -> JSONDict:
        """Construct a terminal execution event."""
        e: JSONDict = {
            "event_type": "terminal_command",
            "command": command,
            "metadata": metadata or {},
        }
        if exit_code is not None:
            e["exit_code"] = exit_code
        if stdout_summary is not None:
            e["stdout_summary"] = stdout_summary
        return e

    @staticmethod
    def git_commit(
        message: str,
        hash: str,
        branch: str | None = None,
        files_changed: list[str] | None = None,
        metadata: JSONDict | None = None,
    ) -> JSONDict:
        """Construct a git commit event."""
        e: JSONDict = {
            "event_type": "git_commit",
            "message": message,
            "hash": hash,
            "files_changed": files_changed or [],
            "metadata": metadata or {},
        }
        if branch is not None:
            e["branch"] = branch
        return e

    @staticmethod
    def diagnostic(
        message: str,
        severity: str = "error",
        source: str = "python",
        file: str | None = None,
        line: int | None = None,
        metadata: JSONDict | None = None,
    ) -> JSONDict:
        """Construct a diagnostic event."""
        e: JSONDict = {
            "event_type": "diagnostic",
            "message": message,
            "severity": severity,
            "source": source,
            "metadata": metadata or {},
        }
        if file is not None:
            e["file"] = file
        if line is not None:
            e["line"] = line
        return e
