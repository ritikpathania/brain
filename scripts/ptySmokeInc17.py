#!/usr/bin/env python3
"""Increment 17 PTY smoke: saved-rule auto-allow end to end.

Config pre-seeds an always-allow rule for bash commands starting with
"git ". The stub daemon emits a tool_use packet plus a
tool_permission_requested frame mid-stream and PARKS until it sees
v1/tool/resolve on a second connection — exactly like the real daemon's
waiter. The shell must auto-allow (notice 'Allowed bash (rule 1)'),
deliver granted=true over the wire (script-level assertion against the
stub's recorded resolutions), never render the permission dialog
(cumulative-buffer absence check is sound: once emitted, always in buf),
and freeze the post-grant answer.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc17-smoke.sock"
CONFIG_FILE = "/tmp/brain-inc17-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
PKG_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell"

RESOLUTIONS = []
RESOLVED = threading.Event()

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto",
               "permissions": {"allow": [{"tool": "bash", "inputPrefix": "git "}]}}, f)

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
                    if not payload and isinstance(req.get("body"), str):
                        try:
                            payload = json.loads(req["body"])
                        except Exception:
                            payload = {}
                    def reply(obj):
                        fobj.write(json.dumps(obj) + "\n")
                        fobj.flush()
                    if act == "v1/session/create":
                        reply({"id": rid, "status": "success",
                               "body": {"session_id": "stub-s17"}})
                    elif act == "v1/generation/stream":
                        reply({"type": "stream_start", "session_id": "stub-s17",
                               "sequence": 0})
                        time.sleep(0.3)
                        reply({"type": "tool_use", "session_id": "stub-s17",
                               "toolUse": {"id": "call-17", "name": "bash",
                                           "input": {"command": "git status"}},
                               "sequence": 1})
                        reply({"type": "tool_permission_requested",
                               "session_id": "stub-s17",
                               "call_id": "call-17", "tool_name": "bash",
                               "input": {"command": "git status"},
                               "reason": "tool execution requires approval",
                               "sequence": 2})
                        # Park like the real waiter until a verdict arrives
                        RESOLVED.wait(timeout=15)
                        reply({"type": "tool_result", "session_id": "stub-s17",
                               "call_id": "call-17", "output": "On branch main",
                               "is_error": False, "exit_code": 0,
                               "sequence": 3})
                        time.sleep(0.2)
                        reply({"type": "token", "session_id": "stub-s17",
                               "token": "Done.", "sequence": 4})
                        reply({"type": "finished", "session_id": "stub-s17",
                               "status": "completed", "sequence": 5})
                    elif act == "v1/tool/resolve":
                        RESOLUTIONS.append({"call_id": payload.get("call_id"),
                                            "granted": payload.get("granted")})
                        RESOLVED.set()
                        reply({"id": rid, "status": "success", "body": {}})
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

# ── Flow B: prompt triggers a ruled tool call -> auto-allow, no dialog ────
os.write(fd, b"check repo status")
pump(0.3)
os.write(fd, b"\r")
ok &= expect("auto-allow-notice", "Allowed bash (rule 1)")

# ── Flow C: verdict reached the stub daemon; the turn resumed and froze ───
deadline = time.time() + 10
while time.time() < deadline and len(RESOLUTIONS) == 0:
    pump(0.1)
wire_ok = RESOLUTIONS == [{"call_id": "call-17", "granted": True}]
print(("PASS" if wire_ok else "FAIL") + " wire-resolution " + json.dumps(RESOLUTIONS))
ok &= wire_ok
ok &= expect("post-grant-answer", "Done.")

# Cumulative buffer: if the dialog EVER rendered, its text would remain
# in buf even after Ink overwrote the screen.
never_shown = "Permission required" not in clean(buf)
print(("PASS" if never_shown else "FAIL") + " dialog-never-shown")
ok &= never_shown

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
