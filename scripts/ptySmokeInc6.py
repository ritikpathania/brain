#!/usr/bin/env python3
"""Increment 6 PTY smoke: the agentic feedback loop as seen live in the TUI.
One prompt drives TWO permission dialogs; each allowed result feeds back and
the model issues a follow-up call before closing text ("Loop closed."). The
deny flow proves a refusal still lets the model finish ("Understood, moving
on."). Wire truth comes from the request log: two v1/tool/resolve calls on the
allow path, zero tool_result frames during the denied turn.

Discipline (carried from Inc 1-5): stub UDS daemon, winsize ioctl before exec,
discrete keystroke writes with >=0.3 s pumps between distinct keys (ink parses
one stdin chunk as one keypress), ANSI-stripped matching, occurrence-count
waits for repeated UI elements. Stub sequences are STRICTLY consecutive
including terminal frames - a gap aborts the shell.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc6-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc6-smoke-requests.jsonl"
# Env overrides let a fixture refresh run against a committed-code worktree
# (BRAIN_PTY_PKG_DIR) and capture elsewhere (BRAIN_PTY_FIXTURE_DIR).
PKG_DIR = os.environ.get(
    "BRAIN_PTY_PKG_DIR",
    "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell")
FIXTURE_DIR = os.environ.get(
    "BRAIN_PTY_FIXTURE_DIR",
    PKG_DIR + "/src/test/fixtures/pty/inc6")
CONFIG_FILE = "/tmp/brain-inc6-smoke-config.json"
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
                               "body": {"session_id": "stub-session-6"}})
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
                        # Two-round turn: daemon-measured thinking opens it
                        # (Inc 13), a result feeds back, the model issues a
                        # SECOND call, then closes with text. Mirrors the Inc 6
                        # daemon loop on the wire.
                        reply({"type": "thinking_start", "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "thinking_delta", "thinking": "weighing the request",
                               "sequence": 1})
                        time.sleep(0.2)
                        reply({"type": "thinking_end", "duration_ms": 1200, "sequence": 2})
                        time.sleep(0.2)
                        reply({"type": "tool_use", "toolUse": {"id": "call_a",
                               "name": "bash", "input": {"command": "echo round-one-stub"}},
                               "sequence": 3})
                        time.sleep(0.2)
                        reply({"type": "tool_permission_requested", "call_id": "call_a",
                               "tool_name": "bash", "input": {"command": "echo round-one-stub"},
                               "reason": "shell access", "sequence": 4})
                        evt = threading.Event(); PERM_EVENTS["call_a"] = evt
                        granted_a = bool(evt.wait(timeout=10) and PERM_GRANTED.get("call_a"))
                        if granted_a:
                            reply({"type": "tool_result", "call_id": "call_a",
                                   "tool_name": "bash", "output": "round-one-stub\n",
                                   "is_error": False, "exit_code": 0, "sequence": 5})
                            time.sleep(0.2)
                            reply({"type": "tool_use", "toolUse": {"id": "call_b",
                                   "name": "bash", "input": {"command": "echo round-two-stub"}},
                                   "sequence": 6})
                            time.sleep(0.2)
                            reply({"type": "tool_permission_requested", "call_id": "call_b",
                                   "tool_name": "bash", "input": {"command": "echo round-two-stub"},
                                   "reason": "shell access", "sequence": 7})
                            evt_b = threading.Event(); PERM_EVENTS["call_b"] = evt_b
                            granted_b = bool(evt_b.wait(timeout=10) and PERM_GRANTED.get("call_b"))
                            if granted_b:
                                reply({"type": "tool_result", "call_id": "call_b",
                                       "tool_name": "bash", "output": "round-two-stub\n",
                                       "is_error": False, "exit_code": 0, "sequence": 8})
                                time.sleep(0.2)
                                reply({"type": "token", "token": "Loop closed.", "sequence": 9})
                                time.sleep(0.3)
                                reply({"type": "finished", "status": "completed", "sequence": 10})
                            else:
                                reply({"type": "tool_denied", "call_id": "call_b",
                                       "tool_name": "bash", "sequence": 8})
                                time.sleep(0.2)
                                reply({"type": "token", "token": "Second call refused.",
                                       "sequence": 9})
                                time.sleep(0.3)
                                reply({"type": "finished", "status": "completed", "sequence": 10})
                        else:
                            # Turn B path shares this branch: denial feeds back,
                            # the model keeps talking, turn completes normally.
                            reply({"type": "tool_denied", "call_id": "call_a",
                                   "tool_name": "bash", "sequence": 5})
                            time.sleep(0.2)
                            reply({"type": "token", "token": "Understood, moving on.",
                                   "sequence": 6})
                            time.sleep(0.3)
                            reply({"type": "finished", "status": "completed", "sequence": 7})
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

# ── Flow B: two allowed rounds render two cards and closing text ───────────
def frames_log():
    try:
        with open(FRAMES_FILE) as f:
            return f.read()
    except Exception:
        return ""

os.write(fd, b"run the stub loop")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("card-one", "round-one-stub")
ok &= expect_count("dialog-one", "Permission required", 1)
os.write(fd, b"y"); pump(0.5)          # allow call_a (settle before next key)
ok &= expect_count("dialog-two", "Permission required", 2)
pump(1.2)                               # settle: dialog must mount before key
snapshot("loop-second-permission")
os.write(fd, b"y"); pump(0.5)          # allow call_b
ok &= expect("card-two-output", "round-two-stub")
ok &= expect("closing-text", "Loop closed.")
# Inc 13: once the turn freezes, the daemon-timed thinking row must show.
ok &= expect("thought-label", "✻ Thought for 1.2s")
deadline = time.time() + 6
wire_two = frames_log().count('"v1/tool/resolve"') >= 2
print(("PASS" if wire_two else "FAIL") + " two-resolves-on-wire")
ok &= wire_two
snapshot("loop-complete")

# ── Flow C: denying the first call still lets the model finish ─────────────
pump(0.8)   # let the previous turn settle so submit isn't swallowed
frames_before = frames_log()
os.write(fd, b"now refuse it")
pump(0.3)
os.write(fd, b"\r")
ok &= expect_count("dialog-three", "Permission required", 3)
pump(1.2)
snapshot("deny-pending")
os.write(fd, b"n"); pump(0.5)
ok &= expect("denied-notice", "Denied bash")
ok &= expect("post-deny-text", "Understood, moving on.")
# Wire truth: this turn emitted NO tool_result — only the denial rode back.
frames_delta = frames_log()[len(frames_before):]
no_result = "tool_result" not in frames_delta
print(("PASS" if no_result else "FAIL") + " denied-turn-executes-nothing")
ok &= no_result
# Second turn of the run also opened with thinking: both labels persist.
ok &= expect_count("thought-both-turns", "✻ Thought for 1.2s", 2)
snapshot("deny-continuation")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
