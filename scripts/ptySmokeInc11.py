#!/usr/bin/env python3
"""Increment 11 PTY smoke: advertised `!` bash mode actually executes.

The stub daemon serves `v1/shell/exec` on short-lived connections: a fast
echo (completed card, daemon duration_ms), a failing command ("Failed ·
exit 1"), and a slow command whose reply loses the race against esc. A
stored session carrying both executed envelopes proves /resume replays the
standalone turns as frozen cards. Discipline carried from Inc 1-9: stub UDS
daemon, winsize ioctl before exec, discrete keystroke writes with >=0.3 s
pumps, ANSI-stripped matching."""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc11-smoke.sock"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc11"
CONFIG_FILE = "/tmp/brain-inc11-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)


def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))


def env(**over):
    return json.dumps({"type": "tool_event", "v": 1, **over})


LOADED_MESSAGES = [
    {"id": "m1", "role": "user", "content": "! echo bang-inc11"},
    {"id": "m2", "role": "tool", "content": env(
        call_id="shell-bang", name="bash", input={"command": "echo bang-inc11"},
        outcome="executed", is_error=False, exit_code=0,
        output="bang-inc11\n", duration_ms=88)},
    {"id": "m3", "role": "user", "content": "! false"},
    {"id": "m4", "role": "tool", "content": env(
        call_id="shell-false", name="bash", input={"command": "false"},
        outcome="executed", is_error=True, exit_code=1,
        output="", duration_ms=15)},
]


def exec_body(rid, cmd, out, is_err, code, ms):
    return {"version": "1.0", "type": "Response", "id": rid,
            "status": "success",
            "body": {"call_id": "shell-%d" % (abs(hash(cmd)) % 10000),
                     "name": "bash", "input": {"command": cmd},
                     "outcome": "executed", "output": out,
                     "is_error": is_err, "exit_code": code,
                     "duration_ms": ms}}


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
                    payload = req.get("payload") or {}
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"version": "1.0", "type": "Response", "id": rid,
                               "status": "success",
                               "body": {"session_id": "stub-session-11"}})
                    elif act == "session/list":
                        reply({"id": rid, "status": "success", "body": {
                            "sessions": [{
                                "session_id": "sess-old-11",
                                "title": "Persisted events demo",
                                "message_count": len(LOADED_MESSAGES),
                                "created_at": NOW_MS // 1000 - 7200,
                                "updated_at": NOW_MS // 1000 - 300,
                            }],
                            "total": 1}})
                    elif act == "v1/session/load":
                        reply({"id": rid, "status": "success", "body": {"session": {
                            "id": "sess-old-11",
                            "title": "Persisted events demo",
                            "archived": False, "pinned": False,
                            "updated_at_ms": NOW_MS - 300_000,
                            "messages": LOADED_MESSAGES}}})
                    elif act == "v1/shell/exec":
                        cmd = str(payload.get("command", ""))
                        # Slow command lets the esc-cancel flow win the race.
                        if cmd == "sleep 5":
                            time.sleep(1.5)
                            reply(exec_body(rid, cmd, "", False, 0, 1500))
                        elif cmd == "false":
                            reply(exec_body(rid, cmd, "", True, 1, 15))
                        else:
                            reply(exec_body(rid, cmd,
                                            cmd.replace("echo ", "") + "\n",
                                            False, 0, 88))
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


def expect_count(label, needle, want, timeout=8.0):
    """Wait until `needle` appears `want` times in the CUMULATIVE screen
    buffer (Inc 6 pattern): plain substring matching cannot tell a fresh
    render from an identical earlier one."""
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

# ── Flow A: boot ───────────────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")
snapshot("boot")

# ── Flow B: successful ! command renders user row + completed card ────────
os.write(fd, b"! echo bang-inc11")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("bang-output", "bang-inc11")
ok &= expect("bang-duration-label", "Done in 0.1s")
pump(0.5)
snapshot("bang")

# ── Flow C: failing ! command renders Failed · exit 1 ─────────────────────
os.write(fd, b"! false")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("failed-exit-code", "Failed · exit 1")
pump(0.5)
snapshot("failed")

# ── Flow D: esc during a slow command cancels the wait, composer freed ────
os.write(fd, b"! sleep 5")
pump(0.4)
os.write(fd, b"\r")
pump(0.6)
os.write(fd, b"\x1b")          # esc -> composer:abort -> controller.abort()
ok &= expect("cancelled-notice", "Shell command cancelled.")
pump(0.5)
snapshot("cancel")

# ── Flow E: /resume replays both commands as frozen cards ─────────────────
os.write(fd, b"/resume")
pump(0.4)
os.write(fd, b"\r")
ok &= expect("picker-listing", "Persisted events demo")
pump(0.5)
snapshot("picker")
os.write(fd, b"\r")
ok &= expect("resume-notice", "Resumed")
ok &= expect("replay-bang-card", "bang-inc11")
ok &= expect_count("replay-failed-cards", "Failed · exit 1", 2)
ok &= expect("replay-user-line", "! echo bang-inc11")
snapshot("resumed")

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

sys.exit(0 if ok else 1)
