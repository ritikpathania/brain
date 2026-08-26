#!/usr/bin/env python3
"""Increment 2 PTY smoke: slash palette, navigation, tab-completion,
/help execution, /clear wipe, and the bash-strip wire regression.

Discipline (carried from ptySmokeInc1.py): stub UDS daemon, winsize ioctl
before exec, discrete keystroke writes with >=0.3 s pumps between distinct
keys (ink parses one stdin chunk as one keypress), ANSI-stripped matching.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc2-smoke.sock"
FRAMES_FILE = "/tmp/brain-inc2-smoke-requests.jsonl"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc2"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

# ── Stub daemon: session-create + instant finished frame (only Flow D talks to it).
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
                    if req.get("action") == "v1/session/create":
                        fobj.write(json.dumps({"id": rid, "status": "success",
                                               "body": {"session_id": "stub-session-2"}}) + "\n")
                        fobj.flush()
                    elif req.get("action") == "v1/generation/stream":
                        fobj.write(json.dumps({"type": "finished", "status": "completed",
                                               "sequence": 0}) + "\n")
                        fobj.flush()
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

# ── Flow A: launch, open palette, navigate ─────────────────────────────────
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")
os.write(fd, b"/")                       # opens palette with ALL commands
# Inc 21: the catalog is the canonical registry — eight commands, name-sorted
# (clear, doctor, help, memory, …), initial selection on the first row.
ok &= expect("palette-listed", "/help")
ok &= expect("palette-desc", "List available slash commands")
os.write(fd, b"\x1b[B")                  # ↓ selection moves to /doctor
ok &= expect("palette-nav-doctor", "❯ /doctor")
os.write(fd, b"\x1b[A")                  # ↑ back to first row
ok &= expect("palette-nav-clear", "❯ /clear")
snapshot("palette")
os.write(fd, b"\x1b")                    # esc closes the menu…
pump(0.3)
os.write(fd, b"\x7f")                    # …then backspace empties '/'
pump(0.3)

# ── Flow B: filtered palette + execute /help via prefix resolution ─────────
os.write(fd, b"/he")
pump(0.3)
ok &= expect("palette-filtered", "/help")
os.write(fd, b"\r")                      # enter submits '/he' → unique prefix → help
ok &= expect("help-header", "Slash commands:")
ok &= expect("help-body", "/quit — Exit Brain shell")
snapshot("executed")

# ── Flow C: tab-completion + /clear wipes the transcript ──────────────────
os.write(fd, b"/cl")
pump(0.3)
os.write(fd, b"\t")                      # completes buffer to '/clear '
pump(0.3)
ok &= expect("tab-completed", "/clear ")
os.write(fd, b"\r")
deadline = time.time() + 8
cleared = False
while time.time() < deadline:
    pump(0.1)
    # Compare only the tail after the last mark render: earlier frames still
    # carry Flow B's help output, but /clear removes it from the live screen.
    tail = clean(buf).split("◆ BRAIN")[-1]
    if "Exit Brain shell" not in tail and "❯" in tail:
        cleared = True
        break
print(("PASS" if cleared else "FAIL") + " clear-executed")
ok &= cleared

# ── Flow D: regression — bash strip still hits the wire bare ──────────────
os.write(fd, b"!echo hi")                # multi-char chunk inserts as text
pump(0.3)
os.write(fd, b"\r")                      # enter is its own keystroke
deadline = time.time() + 8
stripped = False
while time.time() < deadline:
    pump(0.1)
    try:
        with open(FRAMES_FILE) as f:
            for line in f:
                req = json.loads(line)
                # Bash mode has been a real shell-exec since Inc 5; assert
                # the '!' prefix is stripped on the exec wire shape.
                if req.get("action") == "v1/shell/exec" \
                        and (req.get("payload") or {}).get("command", "").strip() == "echo hi":
                    stripped = True
    except FileNotFoundError:
        pass
    if stripped:
        break
print(("PASS" if stripped else "FAIL") + " bash-strip")
ok &= stripped

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
snapshot("cleared")

sys.exit(0 if ok else 1)
