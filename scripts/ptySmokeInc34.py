#!/usr/bin/env python3
"""Increment 34 PTY smoke: Interactive Search Overlay & Seamless Session Navigation.

Proves against a live daemon that:
1. /search <query> opens the SearchOverlayView modal with live match excerpts.
2. Pressing Enter adopts the selected match's session and hydrates its messages.
3. /resume <id_or_prefix> directly resolves and resumes without picker.
4. /resume <ambiguous> opens ResumePicker with candidates.
5. /resume <unknown> produces a clear notice.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc34-pty-")
sock_path = os.path.join(temp_dir, "brain.sock")
db_path = os.path.join(temp_dir, "brain_runtime.db")
pid_path = os.path.join(temp_dir, "brain.pid")

# 1. Start live daemon
daemon_bin = "/Users/ritikpathania/Developer/PyCharm/brain/target/debug/brain-daemon"
daemon_proc = subprocess.Popen(
    [daemon_bin, "daemon", "run"],
    env={
        **os.environ,
        "BRAIN_SOCKET_PATH": sock_path,
        "BRAIN_PID_PATH": pid_path,
        "BRAIN_DB_PATH": db_path,
        "BRAIN_CONFIG_DIR": temp_dir,
        "BRAIN_HEALTH_PORT": "0",
    },
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
)

ready = False
for _ in range(60):
    if os.path.exists(sock_path):
        try:
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(sock_path)
            s.close()
            ready = True
            break
        except Exception:
            pass
    time.sleep(0.1)

if not ready:
    daemon_proc.kill()
    print("FAIL: Daemon did not start in time")
    sys.exit(1)

# Helper for RPC
def rpc(action, body):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(sock_path)
    req = {
        "version": "1.0",
        "type": "Request",
        "id": 1,
        "action": action,
        "body": json.dumps(body)
    }
    s.sendall((json.dumps(req) + "\n").encode("utf-8"))
    f = s.makefile("r")
    line = f.readline()
    s.close()
    frame = json.loads(line)
    if "body" in frame and isinstance(frame["body"], str):
        try:
            return json.loads(frame["body"])
        except Exception:
            return frame["body"]
    return frame.get("body", frame)

# Pre-seed Session A
start_a = rpc("v1/session/start", {"title": "Cryogenic Cooling Architecture"})
sid_a = start_a["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_a,
    "role": "user",
    "content": "Superconducting quantum circuits operate inside cryogenic dilution refrigerators."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_a,
    "assistant_response": "Dilution units achieve temperatures below 15 millikelvin."
})

# Pre-seed Session B
start_b = rpc("v1/session/start", {"title": "Trapped Ion Confinement"})
sid_b = start_b["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_b,
    "role": "user",
    "content": "Trapped ion hardware uses radiofrequency Paul traps."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_b,
    "assistant_response": "Quadrupole potentials provide stable ion confinement."
})

# 2. Launch brain-shell in PTY
pid, fd = pty.fork()
if pid == 0:
    os.environ["BRAIN_SOCKET_PATH"] = sock_path
    os.environ["TERM"] = "xterm-256color"
    os.chdir("/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell")
    os.execvp("bun", ["bun", "run", "src/main.tsx"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = b""
def pump(seconds=0.4):
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

def send_char(ch, delay=0.08):
    os.write(fd, ch.encode("utf-8") if isinstance(ch, str) else ch)
    pump(delay)

def send_text(text, delay=0.05):
    for ch in text:
        send_char(ch, delay)
    send_char("\r", 0.4)

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

# Wait for boot
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# 1. Execute /search cryogenic
send_text("/search cryogenic")
pump(0.6)

# Verify search modal opens and matches are rendered
ok &= expect("search-modal-title", "Search Transcripts")
ok &= expect("search-match-snippet", "cryogenic dilution")

# 2. Press Enter to adopt Session A
send_char("\r", 0.6)

ok &= expect("resumed-session-a", "Resumed “Cryogenic Cooling Architecture”")
ok &= expect("replayed-session-a-msg", "Superconducting quantum circuits")

# 3. Direct resume via exact ID of Session B
send_text(f"/resume {sid_b}")
pump(0.6)

ok &= expect("direct-resumed-session-b", "Resumed “Trapped Ion Confinement”")
ok &= expect("replayed-session-b-msg", "Trapped ion hardware uses radiofrequency Paul traps")

# 4. Ambiguous prefix opens ResumePicker pre-filtered
ambiguous_prefix = sid_a[:6]
send_text(f"/resume {ambiguous_prefix}")
pump(0.6)

ok &= expect("ambiguous-resume-picker", "Resume session")
send_char("\x1b", 0.3) # Close picker with Esc
pump(0.6) # Let prompt input settle

# 5. Unknown target direct resume produces clear notice
send_text("/resume nonexistent_session_id_target")
pump(0.8)

ok &= expect("unknown-resume-notice", "No session found matching “nonexistent_session_id_target”.")

# Close shell
try:
    send_char("\x1b", 0.2)
    send_char("\x03", 0.2)
except Exception:
    pass

try:
    os.kill(pid, signal.SIGKILL)
except Exception:
    pass

try:
    daemon_proc.kill()
    daemon_proc.wait()
except Exception:
    pass

print(f"\nInc 34 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
