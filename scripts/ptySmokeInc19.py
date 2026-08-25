#!/usr/bin/env python3
"""Increment 19 PTY smoke: thinking persistence & replay end to end.

Turn 1 streams REAL thinking frames (start/delta/end, then token +
finished); the live thinking body renders during the turn and freezes
into a "Thought for X.Xs" summary row. Then /resume loads a stored
session whose messages include a thinking_block envelope: the collapsed
summary line renders at its chronological spot while the body text NEVER
appears post-resume (cumulative-buffer absence check is sound: once
emitted, always in buf). The live body IS asserted present — proving the
thinking stream itself actually happened rather than the resume checks
passing vacuously.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc19-smoke.sock"
CONFIG_FILE = "/tmp/brain-inc19-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
PKG_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

LIVE_THINKING = "inc19-live-thinking-marker"
REPLAY_SECRET = "SECRET-REPLAY-BODY-MARKER"


def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))


with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto"}, f)


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
                               "body": {"session_id": "stub-s19"}})
                    elif act == "v1/generation/stream":
                        reply({"type": "stream_start", "session_id": "stub-s19",
                               "sequence": 0})
                        time.sleep(0.2)
                        reply({"type": "thinking_start", "session_id": "stub-s19",
                               "sequence": 1})
                        time.sleep(0.2)
                        reply({"type": "thinking_delta", "session_id": "stub-s19",
                               "thinking": LIVE_THINKING, "text": LIVE_THINKING,
                               "sequence": 2})
                        time.sleep(0.2)
                        reply({"type": "thinking_end", "session_id": "stub-s19",
                               "duration_ms": 800, "sequence": 3})
                        reply({"type": "token", "session_id": "stub-s19",
                               "token": "Live answer.", "sequence": 4})
                        reply({"type": "finished", "session_id": "stub-s19",
                               "status": "completed", "sequence": 5})
                    elif act == "session/list":
                        now_ms = int(time.time() * 1000)
                        reply({"id": rid, "status": "success",
                               "body": {"sessions": [{
                                   "sessionId": "stub-s19", "title": "S19",
                                   "createdAtMs": now_ms, "updatedAtMs": now_ms}],
                                   "total": 1}})
                    elif act == "v1/session/load":
                        now_s = int(time.time())
                        envelope = json.dumps({"type": "thinking_block", "v": 1,
                                               "text": REPLAY_SECRET,
                                               "duration_ms": 800})
                        reply({"id": rid, "status": "success",
                               "body": {"session": {
                                   "id": "stub-s19", "title": "S19",
                                   "created_at_ms": now_s * 1000,
                                   "updated_at_ms": now_s * 1000,
                                   "archived": False, "pinned": False,
                                   "goals": [],
                                   "messages": [
                                       {"id": "m1", "role": "user",
                                        "content": "hello", "timestamp": now_s},
                                       {"id": "m2", "role": "thinking",
                                        "content": envelope, "timestamp": now_s},
                                       {"id": "m3", "role": "assistant",
                                        "content": "Replayed answer.",
                                        "timestamp": now_s}]}}})
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


srv_thread = threading.Thread(target=serve, daemon=True)
srv_thread.start()
time.sleep(0.3)  # let bind() land before the child connects

pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["TERM"] = "xterm-256color"
    os.environ["BRAIN_CONFIG_PATH"] = CONFIG_FILE
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

# ── Flow A: boot ───────────────────────────────────────────────────────────
ok &= expect("welcome-wordmark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# ── Flow B: live turn with a real thinking segment ─────────────────────────
os.write(fd, b"think hard")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("frozen-thought-summary", "Thought for")
ok &= expect("live-answer", "Live answer.")

# ── Flow C: /resume replays the persisted envelope collapsed ──────────────
os.write(fd, b"/resume")
pump(0.3)
os.write(fd, b"\r")
pump(0.5)
os.write(fd, b"\r")  # commit picker selection (index 0)
ok &= expect("resume-notice", "Resumed")
ok &= expect("replay-answer", "Replayed answer.")
ok &= expect("replay-summary-line", "✻ Thought for 0.8s")

# Cumulative buffer: the replayed body must NEVER have been rendered, while
# the live body must have been (else Flow B proved nothing).
never_shown = REPLAY_SECRET not in clean(buf)
print(("PASS" if never_shown else "FAIL") + " replay-body-hidden")
ok &= never_shown
live_seen = LIVE_THINKING in clean(buf)
print(("PASS" if live_seen else "FAIL") + " live-thinking-body-visible-once")
ok &= live_seen

# ── Teardown ───────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
time.sleep(0.5)
try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass
try:
    os.remove(CONFIG_FILE)
except OSError:
    pass

sys.exit(0 if ok else 1)
