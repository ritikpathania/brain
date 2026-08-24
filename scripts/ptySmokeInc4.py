#!/usr/bin/env python3
"""Increment 4 PTY smoke: the full permission round trip — dialog driven by
tool_permission_requested, then the y/n resolution sent back over the wire as
v1/tool/resolve on a SECOND connection (asserted from the request log), with
grant resuming the stream ("Approved.") and deny marking the tool card.

Discipline (carried from Inc 1/2/3): stub UDS daemon, winsize ioctl before
exec, discrete keystroke writes with >=0.3 s pumps between distinct keys
(ink parses one stdin chunk as one keypress), ANSI-stripped matching.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc4-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc4-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc4"
CONFIG_FILE = "/tmp/brain-inc4-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)

PERM_EVENTS = {}    # call_id -> threading.Event
PERM_GRANTED = {}   # call_id -> bool

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
                               "body": {"session_id": "stub-session-4"}})
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
                        # Turn: tool_use -> permission request -> PAUSE until a
                        # v1/tool/resolve arrives on ANOTHER connection (mirrors
                        # the real daemon, whose stream occupies its read loop).
                        reply({"type": "tool_use", "toolUse": {"id": "call_9",
                               "name": "bash", "input": {"command": "ls build"}},
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_9",
                               "tool_name": "bash", "input": {"command": "ls build"},
                               "reason": "shell access", "sequence": 1})
                        evt = threading.Event()
                        PERM_EVENTS["call_9"] = evt
                        resolved = evt.wait(timeout=10)
                        granted = bool(resolved and PERM_GRANTED.get("call_9"))
                        if granted:
                            reply({"type": "token", "token": "Approved.",
                                   "sequence": 2})
                        else:
                            reply({"type": "tool_denied", "call_id": "call_9",
                                   "tool_name": "bash", "sequence": 2})
                        time.sleep(0.3)
                        reply({"type": "finished", "status": "completed", "sequence": 3})
                    elif act == "v1/tool/resolve":
                        payload = req.get("payload", {})
                        cid = payload.get("call_id")
                        PERM_GRANTED[cid] = bool(payload.get("granted"))
                        ev = PERM_EVENTS.get(cid)
                        if ev is not None:
                            ev.set()
                            reply({"type": "resolved", "status": "ok"})
                        else:
                            reply({"type": "Error", "status": "error",
                                   "body": "Unknown or already-resolved tool call"})
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

def expect_count(label, needle, want, timeout=8.0):
    """Wait until `needle` appears `want` times in the CUMULATIVE screen
    buffer. A plain substring expect cannot distinguish a fresh render from
    an identical earlier one (e.g. the second permission dialog)."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if clean(buf).count(needle) >= want:
            print("PASS " + label)
            return True
    print("FAIL %s: %r seen %d times, wanted >=%d"
          % (label, needle, clean(buf).count(needle), want))
    return False

ok = True

# ── Flow A: welcome frame ──────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("welcome-identity", "memory-first agent workspace")
ok &= expect("welcome-hints", "/resume sessions")
ok &= expect("launch-prompt", "❯")
snapshot("welcome")

# ── Flow D1: ALLOW completes the wire round-trip ───────────────────────────
def frames_log():
    try:
        with open(FRAMES_FILE) as f:
            return f.read()
    except Exception:
        return ""

os.write(fd, b"list the build folder")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("tool-card", "bash")
ok &= expect("dialog-header", "Permission required")
snapshot("permissionAllow-pending")
os.write(fd, b"y")                       # allow

# Round-trip criterion: the RESOLUTION reached the wire, not just the UI.
deadline = time.time() + 6
wire_allow = False
while time.time() < deadline:
    pump(0.1)
    if '"v1/tool/resolve"' in frames_log():
        wire_allow = True
        break
print(("PASS" if wire_allow else "FAIL") + " resolve-on-wire")
ok &= wire_allow
ok &= expect("allowed-notice", "Allowed bash")
ok &= expect("approved-token", "Approved.")
snapshot("permissionAllow-done")

# ── Flow D2: DENY marks the card and reports granted=false ─────────────────
pump(0.8)   # let the first turn settle so submit() isn't swallowed by busy
os.write(fd, b"now delete it")
pump(0.3)
os.write(fd, b"\r")
# A second "Permission required" must FRESHLY render before `n` is sent:
# sending earlier races the stub's 200 ms pre-permission sleep and the
# keystroke lands in the (still active) composer instead of the dialog.
ok &= expect_count("dialog-header-2", "Permission required", 2)
snapshot("permissionDeny-pending")
os.write(fd, b"n")                       # deny

deadline = time.time() + 6
wire_deny = False
while time.time() < deadline:
    pump(0.1)
    if '"granted": false' in frames_log() or '"granted":false' in frames_log():
        wire_deny = True
        break
print(("PASS" if wire_deny else "FAIL") + " deny-on-wire")
ok &= wire_deny
ok &= expect("denied-notice", "Denied bash")
snapshot("permissionDeny-done")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
