#!/usr/bin/env python3
"""Increment 1 PTY smoke: launch frame / mid-stream / expanded tool card / bash strip."""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc1-smoke.sock"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
FRAMES_FILE = "/tmp/brain-inc1-smoke-requests.jsonl"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

# ── stub daemon ────────────────────────────────────────────────────────────
STREAM_FRAMES = [
    {"type": "thinking", "thinking": "Recalling memories…"},
    {"type": "thinking", "thinking": " Drafting."},
    {"type": "token", "token": "Hello "},
    {"type": "token", "token": "from the "},
    {"type": "token", "token": "Brain daemon stream."},
    {"type": "tool_use", "toolUse": {"id": "call_1", "name": "read_file",
                                     "input": {"path": "/tmp/brain-demo.txt"}}},
    {"type": "token", "token": " Read the demo file fine."},
]

def serve():
    if os.path.exists(SOCK):
        os.remove(SOCK)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(SOCK)
    srv.listen(8)
    while True:
        conn, _ = srv.accept()
        fobj = conn.makefile("rw")
        def handle(conn=conn, fobj=fobj):
            seq = 0
            try:
                for line in fobj:
                    req = json.loads(line)
                    with open(FRAMES_FILE, "a") as log:
                        log.write(json.dumps(req) + "\n")
                    action = req.get("action")
                    rid = req.get("id")
                    if action == "v1/session/create":
                        fobj.write(json.dumps({"id": rid, "status": "success",
                                               "body": {"session_id": "stub-session-1"}}) + "\n")
                        fobj.flush()
                    elif action == "v1/generation/stream":
                        for i, frame in enumerate(STREAM_FRAMES):
                            out = dict(frame); out["sequence"] = seq; seq += 1
                            fobj.write(json.dumps(out) + "\n"); fobj.flush()
                            if frame["type"] == "tool_use":
                                time.sleep(1.2)          # window for mid-stream asserts
                        out = {"type": "finished", "status": "completed",
                               "sequence": seq}; seq += 1
                        fobj.write(json.dumps(out) + "\n"); fobj.flush()
            except Exception:
                pass
            finally:
                try: conn.close()
                except Exception: pass
        threading.Thread(target=handle, daemon=True).start()

threading.Thread(target=serve, daemon=True).start()
if os.path.exists(FRAMES_FILE):
    os.remove(FRAMES_FILE)

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["BRAIN_KEY_DEBUG"] = "1"
    os.dup2(os.open("/tmp/inc1-keydebug.log", os.O_WRONLY | os.O_CREAT | os.O_TRUNC), 2)
    os.environ["TERM"] = "xterm-256color"
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

FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc1"

def snapshot(name):
    os.makedirs(FIXTURE_DIR, exist_ok=True)
    open(f"{FIXTURE_DIR}/{name}.txt", "w").write(clean(buf))

def expect(label, needle, timeout=8.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print(f"PASS {label}")
            return True
    print(f"FAIL {label}: {needle!r} not seen")
    return False

ok = True

# ── Flow A: launch frame ──────────────────────────────────────────────────
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-tagline", "memory-first agent workspace")
ok &= expect("launch-composer", "❯")
snapshot("launch")

# ── Flow B: mid-stream turn ───────────────────────────────────────────────
# The stub sleeps 1.2 s AFTER the tool_use frame, giving a deterministic
# mid-turn window. The live spinner's tool label can ONLY exist during that
# window — after finishTurn the live region unmounts and the frozen card
# renders 'Done' instead — so it doubles as proof that the stream was
# observed mid-flight rather than after completion.
# Type text and Enter as SEPARATE writes: ink parses one stdin chunk as one
# keypress, so a trailing \r inside the same write is treated as literal
# text, never as the return key.
os.write(fd, b"tell me something")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("mid-stream-thinking", "Recalling memories…")
ok &= expect("mid-stream-text", "Hello from the Brain daemon stream.")
ok &= expect("live-tool-label", "read_file…")

# Turn completes: frozen transcript carries the merged answer + done card.
ok &= expect("final-frozen-answer", "Read the demo file fine.")
ok &= expect("tool-card-collapsed-done", "Done")

# ── Flow C: ctrl+o expands frozen tool cards ──────────────────────────────
os.write(fd, b"\x0f")
ok &= expect("tool-card-expanded", '"path": "/tmp/brain-demo.txt"')
snapshot("expanded")
os.write(fd, b"\x0f")  # restore collapsed
pump(0.3)              # discrete-keypress pacing: coalesced chunks would
                       # make ink parse "\x0f!echo hi" as one paste-like insert

# ── Flow D: bash-mode strip ('!echo hi' submits bare 'echo hi') ───────────
# Runs last: the composer ignores input while busy, and the previous turn
# has fully completed by now.
os.write(fd, b"!echo hi")
pump(0.3)
os.write(fd, b"\r")
deadline = time.time() + 8
stripped = False
while time.time() < deadline:
    pump(0.1)
    try:
        for line in open(FRAMES_FILE):
            req = json.loads(line)
            msgs = (req.get("payload") or {}).get("messages") or []
            if msgs and isinstance(msgs[-1].get("content"), str) and msgs[-1]["content"].strip() == "echo hi":
                stripped = True
    except FileNotFoundError:
        pass
    if stripped:
        break
print(("PASS" if stripped else "FAIL") + " bash-strip")
ok &= stripped

os.write(fd, b"\x03")   # ctrl+c
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
snapshot("transcript")

sys.exit(0 if ok else 1)
