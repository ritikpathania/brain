import socket
from pathlib import Path

from brain_sdk.errors import BrainConnectionError, BrainTimeoutError


class UdsTransport:
    """Manages connection, writing, and NDJSON streaming over a Unix Domain Socket."""

    def __init__(self, socket_path: Path, timeout: float = 5.0):
        self.socket_path = socket_path
        self.timeout = timeout
        self.sock: socket.socket | None = None
        self.read_file = None

    def connect(self):
        """Establish connection to the daemon socket."""
        try:
            self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.sock.settimeout(self.timeout)
            self.sock.connect(str(self.socket_path))
            self.read_file = self.sock.makefile("r", encoding="utf-8")
        except TimeoutError as e:
            self.close()
            raise BrainTimeoutError(f"Connection to daemon timed out: {e}") from e
        except Exception as e:
            self.close()
            raise BrainConnectionError(
                f"Failed to connect to daemon socket at {self.socket_path}: {e}"
            ) from e

    def write_line(self, line: str):
        """Send a single raw string line to the socket."""
        if not self.sock:
            raise BrainConnectionError("Not connected")
        try:
            # Strip potential training newlines to avoid duplicate empty frames
            payload = line.rstrip("\n") + "\n"
            self.sock.sendall(payload.encode("utf-8"))
        except TimeoutError as e:
            raise BrainTimeoutError(f"Write to socket timed out: {e}") from e
        except Exception as e:
            raise BrainConnectionError(f"Failed to write to socket: {e}") from e

    def read_line(self) -> str:
        """Block until a full newline-terminated line is read from the socket."""
        if not self.read_file:
            raise BrainConnectionError("Not connected")
        try:
            line = self.read_file.readline()
            if not line:
                raise BrainConnectionError("Connection closed by daemon (EOF)")
            return line
        except TimeoutError as e:
            raise BrainTimeoutError(f"Read from socket timed out: {e}") from e
        except Exception as e:
            raise BrainConnectionError(f"Failed to read from socket: {e}") from e

    def close(self):
        """Clean up socket and helper stream file handles."""
        if self.read_file:
            try:
                self.read_file.close()
            except Exception:
                pass
            self.read_file = None
        if self.sock:
            try:
                self.sock.close()
            except Exception:
                pass
            self.sock = None
