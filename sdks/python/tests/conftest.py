import os
import socket
import tempfile
import threading

import pytest


@pytest.fixture
def temp_socket_path():
    fd, path = tempfile.mkstemp(suffix=".sock", prefix="brain-test-py-")
    os.close(fd)
    os.remove(path)
    yield path
    if os.path.exists(path):
        try:
            os.remove(path)
        except Exception:
            pass

class MockUdsServer:
    def __init__(self, path):
        self.path = path
        self.listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.listener.bind(path)
        self.listener.listen(1)
        self.thread = None
        self.running = False

    def start(self, handler):
        self.running = True
        def loop():
            self.listener.settimeout(0.1)
            while self.running:
                try:
                    conn, _ = self.listener.accept()
                    handler(conn)
                except TimeoutError:
                    continue
                except Exception:
                    break
        self.thread = threading.Thread(target=loop, daemon=True)
        self.thread.start()

    def stop(self):
        self.running = False
        if self.thread:
            self.thread.join(timeout=1.0)
        try:
            self.listener.close()
        except Exception:
            pass
