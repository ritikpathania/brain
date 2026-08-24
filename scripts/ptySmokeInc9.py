#!/usr/bin/env python3
"""Increment 9 PTY smoke: persisted tool events render as frozen transcript
cards on /resume. The stub daemon serves a stored session whose history
carries three Inc 8 envelopes (executed-ok, executed-failed exit 2, denied)
plus one non-envelope tool message. Discipline carried from Inc 1-6: stub UDS
daemon, winsize ioctl before exec, discrete keystroke writes with >=0.3 s
pumps, ANSI-stripped matching."""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc9-smoke.sock"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc9"
CONFIG_FILE = "/tmp/brain-inc9-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)


def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))


def env(**over):
    return json.dumps({"type": "tool_event", "v": 1, **over})


LOADED_MESSAGES = [
    {"id": "m1", "role": "user", "content": "tidy the workspace"},
    {"id": "m2", "role": "tool", "content": env(
        call_id="c-ok", name="bash", input={"command": "echo resumed-ok"},
        outcome="executed", is_error=False, exit_code=0,
        output="resumed-ok\n", duration_ms=120)},
    {"id": "m3", "role": "tool", "content": env(
        call_id="c-bad", name="bash", input={"command": "false"},
        outcome="executed", is_error=True, exit_code=2,
        output="boom", duration_ms=40)},
    {"id": "m4", "role": "tool", "content": env(
        call_id="c-deny", name="bash", input={"command": "rm -rf /tmp/x"},
        outcome="denied")},
    {"id": "m5", "role": "tool", "content": "legacy free-form note"},
    {"id": "m6", "role": "assistant", "content": "Workspace settled."},
]


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
                    rid = req.get("id")
                    act = req.get("action")
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-session-9"}})
                    elif act == "session/list":
                        reply({"id": rid, "status": "success", "body": {
                            "sessions": [{
                                "session_id": "sess-old-9",
                                "title": "Persisted events demo",
                                "message_count": len(LOADED_MESSAGES),
                                "created_at": NOW_MS // 1000 - 7200,
                                "updated_at": NOW_MS // 1000 - 300,
                            }],
                            "total": 1}})
                    elif act == "v1/session/load":
                        reply({"id": rid, "status": "success", "body": {"session": {
                            "id": "sess-old-9",
                            "title": "Persisted events demo",
                            "archived": False, "pinned": False,
                            "updated_at_ms": NOW_MS - 300_000,
                            "messages": LOADED_MESSAGES}}})
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

# ── Flow A: boot, open /resume, accept the highlighted session ─────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")
snapshot("boot")

os.write(fd, b"/resume")
pump(0.4)
os.write(fd, b"\r")           # submit the slash command -> picker mounts
ok &= expect("picker-listing", "Persisted events demo")
pump(0.5)
snapshot("picker")
os.write(fd, b"\r")           # accept highlighted entry -> resumeSession

# ── Flow B: replayed transcript shows frozen cards, not JSON blobs ─────────
ok &= expect("resume-notice", "Resumed")
ok &= expect("completed-card-output", "resumed-ok")
ok &= expect("failed-card-exit-code", "Failed · exit 2")
ok &= expect("denied-card-label", "Permission denied")
ok &= expect("malformed-falls-back-to-system-row", "legacy free-form note")
ok &= expect("assistant-history-intact", "Workspace settled.")
# The envelope's raw JSON fields must never leak into the transcript.
screen = clean(buf)
raw_leak = '"call_id"' in screen or '"outcome"' in screen
print(("PASS" if not raw_leak else "FAIL") + " envelope-json-not-rendered")
ok &= not raw_leak
snapshot("resumed-transcript")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
