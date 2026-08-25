#!/usr/bin/env python3
"""Increment 15 PTY smoke: daemon outage lifecycle end to end.

The stub daemon is ABSENT when the TUI launches. The first submit fails
with the 'Connection lost — reconnecting…' banner and the status bar
shows the reconnecting segment; the second submit queues visibly. Then
the daemon appears, the monitor restores, and the queued prompt
auto-fires through the normal turn pipeline and its answer freezes into
the transcript. Cumulative-buffer caveat: no absence checks — positive
assertions only.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc15-smoke.sock"
CONFIG_FILE = "/tmp/brain-inc15-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
PKG_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

serve_started = threading.Event()

def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    serve_started.set()
    while True:
        conn, _ = srv.accept()
        def handle(conn=conn):
            fobj = conn.makefile("rw")
            try:
                for line in fobj:
                    req = json.loads(line)
                    rid = req.get("id")
                    act = req.get("action")
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-s15"}})
                    elif act == "v1/generation/stream":
                        # Minimal clean turn: greet and finish. Sequence
                        # numbers strictly consecutive.
                        reply({"type": "stream_start", "session_id": "stub-s15",
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "token", "session_id": "stub-s15",
                               "token": "Daemon is back.", "sequence": 1})
                        time.sleep(0.2)
                        reply({"type": "finished", "session_id": "stub-s15",
                               "status": "completed", "sequence": 2})
                    else:
                        reply({"id": rid, "status": "success", "body": {}})
            except Exception:
                pass
            finally:
                try:
                    conn.close()
                except Exception:
                    pass
        threading.Thread(target=handle, daemon=True).start()

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = CONFIG_FILE
    if os.path.exists(CONFIG_FILE):
        os.remove(CONFIG_FILE)
    os.chdir(PKG_DIR)
    os.execvp("bun", ["bun", "run", "src/main.tsx"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = b""
def pump(seconds):
    global buf
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if fd in r:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk

def expect(label, needle, timeout=10.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    return False

ok = True

# ── Flow A: TUI boots fine with the daemon absent ──────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# ── Flow B: first submit fails loudly and arms the monitor ────────────────
os.write(fd, b"hello there")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("loss-banner", "Connection lost — reconnecting")
ok &= expect("statusbar-segment", "reconnecting (attempt")

# ── Flow C: the next prompt queues visibly instead of vanishing ───────────
os.write(fd, b"are you still there")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("queued-row", "queued — will send on reconnect")

# ── Flow D: daemon appears -> restore -> queued prompt auto-fires ─────────
time.sleep(1.0)   # let a couple of failed probes land first
t = threading.Thread(target=serve, daemon=True)
t.start()
if not serve_started.wait(timeout=5):
    print("FAIL stub-server-up")
    ok = False
ok &= expect("replay-user-row", "are you still there")
ok &= expect("replay-answer", "Daemon is back.")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
