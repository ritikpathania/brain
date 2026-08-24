#!/usr/bin/env python3
"""Increment 3 PTY smoke: welcome frame, /theme picker, /resume picker with
replay, and the permission dialog driven by tool_permission_requested.

Discipline (carried from Inc 1/2): stub UDS daemon, winsize ioctl before
exec, discrete keystroke writes with >=0.3 s pumps between distinct keys
(ink parses one stdin chunk as one keypress), ANSI-stripped matching.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc3-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc3-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc3"
CONFIG_FILE = "/tmp/brain-inc3-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    while True:
        conn, _ = srv.accept()
        def handle(conn=conn):
            fobj = conn.makefile("rw")
            try:
                for line in fobj:
                    req = json.loads(line)
                    with open(FRAMES_FILE, "a") as log:
                        log.write(json.dumps(req) + "\n")
                    rid = req.get("id")
                    act = req.get("action")
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-session-3"}})
                    elif act == "session/list":
                        reply({"id": rid, "status": "success", "body": {
                            "sessions": [{
                                "session_id": "sess-old-9",
                                "title": "Refactor graph indexer",
                                "message_count": 2,
                                "created_at": NOW_MS // 1000 - 7200,
                                "updated_at": NOW_MS // 1000 - 300,
                            }],
                            "total": 1}})
                    elif act == "v1/session/load":
                        reply({"id": rid, "status": "success", "body": {"session": {
                            "id": "sess-old-9",
                            "title": "Refactor graph indexer",
                            "archived": False, "pinned": False,
                            "updated_at_ms": NOW_MS - 300_000,
                            "messages": [
                                {"id": "m1", "role": "user", "content": "index the graph"},
                                {"id": "m2", "role": "assistant", "content": "Indexed 42 nodes."},
                            ]}}})
                    elif act == "v1/generation/stream":
                        # Turn: tool call -> permission request -> (resolution is
                        # local) -> completion. Delay `finished` so the smoke can
                        # answer the dialog while the turn is still open.
                        reply({"type": "tool_use", "toolUse": {"id": "call_9",
                               "name": "bash", "input": {"command": "ls build"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_9",
                               "tool_name": "bash", "input": {"command": "ls build"},
                               "reason": "shell access", "sequence": 1})
                        time.sleep(1.5)
                        reply({"type": "finished", "status": "completed", "sequence": 2})
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

threading.Thread(target=serve, daemon=True).start()
if os.path.exists(FRAMES_FILE):
    os.remove(FRAMES_FILE)

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = CONFIG_FILE
    if os.path.exists(CONFIG_FILE):
        os.remove(CONFIG_FILE)
    os.chdir("/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell")
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

def snapshot(name):
    os.makedirs(FIXTURE_DIR, exist_ok=True)
    with open(os.path.join(FIXTURE_DIR, name + ".txt"), "w") as f:
        f.write(clean(buf))

def expect(label, needle, timeout=8.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    return False

ok = True

# ── Flow A: welcome frame ──────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("welcome-identity", "memory-first agent workspace")
ok &= expect("welcome-hints", "/resume sessions")
ok &= expect("launch-prompt", "❯")
snapshot("welcome")

# ── Flow B: /theme picker, live preview, commit ────────────────────────────
os.write(fd, b"/theme")                  # one chunk inserts as text
pump(0.3)
os.write(fd, b"\r")                      # enter submits
ok &= expect("theme-title", "Theme")
ok &= expect("theme-auto-entry", "Auto (detect terminal)")
os.write(fd, b"\x1b[B")                  # ↓ moves selection (preview switches)
pump(0.3)
ok &= expect("theme-selection-moved", "❯ Dark")
snapshot("theme")
os.write(fd, b"\r")                      # commit → persists to BRAIN_CONFIG_PATH
deadline = time.time() + 5
committed = False
while time.time() < deadline:
    pump(0.1)
    try:
        with open(CONFIG_FILE) as f:
            if json.load(f).get("theme") == "dark":
                committed = True
                break
    except Exception:
        pass
print(("PASS" if committed else "FAIL") + " theme-persisted")
ok &= committed
pump(0.5)

# ── Flow C: /resume picker + transcript replay ─────────────────────────────
os.write(fd, b"/resume")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("resume-title", "Resume session")
ok &= expect("resume-entry", "Refactor graph indexer")
snapshot("resume")
os.write(fd, b"\r")                      # pick it
ok &= expect("resume-replayed-user", "index the graph")
ok &= expect("resume-replayed-assistant", "Indexed 42 nodes.")
ok &= expect("resume-notice", "Resumed")

# ── Flow D: permission dialog over a streamed tool call ────────────────────
os.write(fd, b"list the build folder")   # plain prompt (multi-char paste is fine)
pump(0.3)
os.write(fd, b"\r")
ok &= expect("tool-card", "bash")
ok &= expect("dialog-header", "Permission required")
ok &= expect("dialog-options", "[ Allow ]")
snapshot("permission")
os.write(fd, b"y")                       # allow
ok &= expect("allowed-notice", "Allowed bash")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
