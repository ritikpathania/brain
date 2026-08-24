#!/usr/bin/env python3
"""Increment 9 PTY smoke: persisted tool events render as frozen transcript
cards on /resume. The stub daemon serves a stored session whose history
carries three Inc 8 envelopes (executed-ok, executed-failed exit 2, denied)
plus one non-envelope tool message. Discipline carried from Inc 1-6: stub UDS
daemon, winsize ioctl before exec, discrete keystroke writes with >=0.3 s
pumps, ANSI-stripped matching.

Inc 10 adds a LIVE turn before the resume: two allowed bash calls, the first
completing (daemon-measured duration_ms -> "Done in 0.1s") and the second
failing (exit_code 2 -> "Failed · exit 2"), proving the running card carries
the same facts its replayed twin will show."""
import fcntl, json, os, pty, re, select, signal, socket, struct, sys, termios, threading, time

ROWS, COLS = 30, 100
SOCK = "/tmp/brain-inc9-smoke.sock"
FIXTURE_DIR = "/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell/src/test/fixtures/pty/inc9"
CONFIG_FILE = "/tmp/brain-inc9-smoke-config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")
NOW_MS = int(time.time() * 1000)

PERM_EVENTS = {}    # call_id -> threading.Event
PERM_GRANTED = {}   # call_id -> bool


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
                    elif act == "v1/generation/stream":
                        # Inc 10 live turn: one completing call (duration_ms
                        # measured daemon-side) then one failing call. Frames
                        # strictly consecutive on EVERY path; a gap aborts
                        # the shell.
                        nxt = [0]

                        def frame(obj):
                            obj["sequence"] = nxt[0]
                            nxt[0] += 1
                            reply(obj)

                        frame({"type": "tool_use", "toolUse": {"id": "call-live-ok",
                               "name": "bash", "input": {"command": "echo live-ok"}}})
                        time.sleep(0.2)
                        frame({"type": "tool_permission_requested",
                               "call_id": "call-live-ok", "tool_name": "bash",
                               "input": {"command": "echo live-ok"},
                               "reason": "shell access"})
                        evt = threading.Event()
                        PERM_EVENTS["call-live-ok"] = evt
                        granted = bool(evt.wait(timeout=10)
                                       and PERM_GRANTED.get("call-live-ok"))
                        if granted:
                            frame({"type": "tool_result", "call_id": "call-live-ok",
                                   "tool_name": "bash", "output": "live-ok\n",
                                   "is_error": False, "exit_code": 0,
                                   "duration_ms": 137})
                            time.sleep(0.2)
                            frame({"type": "tool_use", "toolUse": {"id": "call-live-bad",
                                   "name": "bash", "input": {"command": "false"}}})
                            time.sleep(0.2)
                            frame({"type": "tool_permission_requested",
                                   "call_id": "call-live-bad", "tool_name": "bash",
                                   "input": {"command": "false"},
                                   "reason": "shell access"})
                            evt_b = threading.Event()
                            PERM_EVENTS["call-live-bad"] = evt_b
                            granted_b = bool(evt_b.wait(timeout=10)
                                             and PERM_GRANTED.get("call-live-bad"))
                            if granted_b:
                                frame({"type": "tool_result", "call_id": "call-live-bad",
                                       "tool_name": "bash", "output": "boom",
                                       "is_error": True, "exit_code": 2,
                                       "duration_ms": 40})
                                closing = "Live turn done."
                            else:
                                frame({"type": "tool_denied",
                                       "call_id": "call-live-bad",
                                       "tool_name": "bash"})
                                closing = "Refused."
                        else:
                            frame({"type": "tool_denied",
                                   "call_id": "call-live-ok",
                                   "tool_name": "bash"})
                            closing = "Refused."
                        frame({"type": "token", "token": closing})
                        time.sleep(0.3)
                        frame({"type": "finished", "status": "completed"})
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
    dialog render from an identical earlier one."""
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

# ── Flow A2 (Inc 10): LIVE turn — running cards carry the daemon clock ─────
os.write(fd, b"break things")
pump(0.3)
os.write(fd, b"\r")
ok &= expect_count("live-dialog-one", "Permission required", 1)
pump(1.2)                               # settle: dialog must mount before key
os.write(fd, b"y"); pump(0.5)          # allow call-live-ok
ok &= expect_count("live-dialog-two", "Permission required", 2)
pump(1.2)
os.write(fd, b"y"); pump(0.5)          # allow call-live-bad
# Card rows freeze in when the turn ends (current shell behaviour), so both
# daemon-measured labels are asserted against the settled live transcript,
# before any /resume replay has run.
ok &= expect("live-completed-duration", "Done in 0.1s")
ok &= expect("live-failed-exit-code", "Failed · exit 2")
ok &= expect("live-closing-text", "Live turn done.")
snapshot("live-transcript")

# ── Flow B: open /resume, accept the highlighted session ───────────────────
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
