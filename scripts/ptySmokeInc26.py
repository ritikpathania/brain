#!/usr/bin/env python3
"""Increment 26 PTY smoke: Resume Truthfulness.

Validates that /resume overlay:
1. Renders '★ ' for pinned sessions.
2. Orders pinned sessions above unpinned sessions.
3. Excludes archived sessions completely.

Contract: ★ pinned -> unpinned -> archived absent.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc26-smoke.sock"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

# ── Stub UDS daemon returning pinned, unpinned, and archived sessions
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
                    if not line.strip():
                        continue
                    req = json.loads(line)
                    rid = req.get("id")
                    action = req.get("action")
                    if action in ("v1/session/create", "session/create"):
                        fobj.write(json.dumps({
                            "id": rid,
                            "status": "success",
                            "body": {"session_id": "live-session-inc26", "title": "Current Session"}
                        }) + "\n")
                        fobj.flush()
                    elif action in ("v1/session/list", "session/list"):
                        now_ms = int(time.time() * 1000)
                        fobj.write(json.dumps({
                            "id": rid,
                            "status": "success",
                            "body": {
                                "sessions": [
                                    {
                                        "sessionId": "sess-pinned",
                                        "title": "Alpha Pinned",
                                        "messageCount": 5,
                                        "createdAtMs": now_ms - 600000,
                                        "updatedAtMs": now_ms - 300000,
                                        "pinned": True,
                                        "archived": False,
                                    },
                                    {
                                        "sessionId": "sess-standard",
                                        "title": "Beta Standard",
                                        "messageCount": 2,
                                        "createdAtMs": now_ms - 1200000,
                                        "updatedAtMs": now_ms - 60000,
                                        "pinned": False,
                                        "archived": False,
                                    },
                                    {
                                        "sessionId": "sess-archived",
                                        "title": "Gamma Archived",
                                        "messageCount": 1,
                                        "createdAtMs": now_ms - 1800000,
                                        "updatedAtMs": now_ms - 900000,
                                        "pinned": False,
                                        "archived": True,
                                    },
                                ],
                                "total": 3
                            }
                        }) + "\n")
                        fobj.flush()
                    elif action in ("v1/generation/stream", "generation/stream"):
                        fobj.write(json.dumps({
                            "type": "finished",
                            "status": "completed",
                            "sequence": 0
                        }) + "\n")
                        fobj.flush()
                    else:
                        fobj.write(json.dumps({"id": rid, "status": "success", "body": {}}) + "\n")
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
time.sleep(0.2)

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

# 1. Wait for shell to boot
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# 2. Type /resume and press Enter
os.write(fd, b"/resume")
pump(0.3)
os.write(fd, b"\r")
pump(0.5)

# 3. Assert /resume overlay appeared
ok &= expect("resume-overlay", "Resume session")

# 4. Assert pinned session rendered with ★ badge
ok &= expect("pinned-badge", "★ Alpha Pinned")

# 5. Assert standard session rendered
ok &= expect("standard-session", "Beta Standard")

# 6. Check ordering: Alpha Pinned appears before Beta Standard
screen = clean(buf)
idx_pinned = screen.find("Alpha Pinned")
idx_standard = screen.find("Beta Standard")
ordering_ok = idx_pinned != -1 and idx_standard != -1 and idx_pinned < idx_standard
print(("PASS" if ordering_ok else "FAIL") + f" pinned-sorted-first (pinned={idx_pinned}, standard={idx_standard})")
ok &= ordering_ok

# 7. Check archived exclusion: Gamma Archived must NOT appear
archived_absent = "Gamma Archived" not in screen
print(("PASS" if archived_absent else "FAIL") + " archived-excluded")
ok &= archived_absent

# ── Teardown ──
try:
    os.write(fd, b"\x1b")
    pump(0.2)
    os.write(fd, b"\x03")
    pump(0.2)
except Exception:
    pass

try:
    os.kill(pid, signal.SIGKILL)
except ProcessLookupError:
    pass

print(f"\nInc 26 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
