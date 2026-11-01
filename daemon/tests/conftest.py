"""Hermetic fixtures for daemon Python integration tests."""

import json
import os
import socket
import subprocess
import tempfile
import time
import uuid
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass
class UdsRuntime:
    """An isolated foreground daemon and its versioned UDS request helper."""

    socket_path: Path
    _request_id: int = 0

    def call(self, action: str, body: str) -> dict[str, object]:
        self._request_id += 1
        request = {
            "version": "1.0",
            "type": "Request",
            "id": self._request_id,
            "action": action,
            "body": body,
        }

        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
            client.settimeout(2)
            client.connect(str(self.socket_path))
            client.sendall(f"{json.dumps(request)}\n".encode())
            response = _read_json_line(client)

        assert response["id"] == self._request_id
        return response


def _read_json_line(client: socket.socket) -> dict[str, object]:
    buffer = b""
    while b"\n" not in buffer:
        chunk = client.recv(4096)
        if not chunk:
            raise AssertionError("daemon closed the UDS connection without a response")
        buffer += chunk
    return json.loads(buffer.split(b"\n", 1)[0].decode())


def _daemon_binary() -> Path:
    configured = os.environ.get("BRAIN_DAEMON_BIN")
    if configured:
        candidate = Path(configured)
    else:
        candidate = (
            Path(__file__).resolve().parents[2] / "target" / "debug" / "brain-daemon"
        )

    if not candidate.is_file():
        pytest.fail(
            "BRAIN_DAEMON_BIN must identify a built brain-daemon binary; "
            "build it with `cargo build -p brain-daemon` before running "
            "Python UDS tests"
        )
    return candidate


@pytest.fixture(scope="session")
def uds_runtime(tmp_path_factory: pytest.TempPathFactory) -> Iterator[UdsRuntime]:
    """Runs one daemon in a unique runtime directory and always reaps it."""

    runtime_dir = tmp_path_factory.mktemp("brain-python-uds")
    # macOS limits Unix socket paths to SUN_LEN bytes; pytest's temp directory
    # intentionally exceeds that limit on many runners. Keep only the socket
    # under system temp directory while all persistent runtime state stays
    # in runtime_dir.
    base_tmp = Path(os.environ.get("TMPDIR") or tempfile.gettempdir())
    socket_path = base_tmp / f"bpy-{os.getpid()}-{uuid.uuid4().hex[:8]}.sock"
    log_path = runtime_dir / "daemon.log"
    env = {
        **os.environ,
        "HOME": str(runtime_dir),
        "XDG_CONFIG_HOME": str(runtime_dir),
        "XDG_DATA_HOME": str(runtime_dir),
        "BRAIN_CONFIG_DIR": str(runtime_dir),
        "BRAIN_SOCKET_PATH": str(socket_path),
        "BRAIN_PID_PATH": str(runtime_dir / "brain.pid"),
        "BRAIN_LOG_PATH": str(log_path),
        "BRAIN_DB_PATH": str(runtime_dir / "brain.db"),
        "BRAIN_HEALTH_PORT": "0",
    }
    log_file = log_path.open("w")
    process = subprocess.Popen(
        [_daemon_binary(), "daemon", "run"],
        cwd=Path(__file__).resolve().parents[2],
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=log_file,
        stderr=subprocess.STDOUT,
        text=True,
    )
    runtime = UdsRuntime(socket_path)
    response = None
    ready = False

    try:
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if process.poll() is not None:
                break
            if socket_path.exists():
                try:
                    response = runtime.call("v1/status", "")
                except (AssertionError, OSError, TimeoutError):
                    time.sleep(0.05)
                else:
                    if (
                        response.get("type") == "Response"
                        and response.get("status") == "success"
                    ):
                        ready = True
                        break

        if process.poll() is not None or not ready:
            log_file.flush()
            pytest.fail(
                "isolated daemon did not become ready; "
                f"exit={process.poll()}, log={log_path.read_text()}"
            )

        yield runtime
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        log_file.close()
        socket_path.unlink(missing_ok=True)
