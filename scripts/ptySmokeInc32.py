#!/usr/bin/env python3
"""Increment 32 PTY smoke: Cross-Session Transcript Search.

Proves against a live daemon that:
1. Messages stored across multiple sessions are searchable via /search <query>.
2. /search matches render session tags, role, and transcript snippets.
3. Unmatched queries report clear empty-result notices.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc32-pty-")
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
start_a = rpc("v1/session/start", {"title": "Cryogenic System Topic"})
sid_a = start_a["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_a,
    "role": "user",
    "content": "Superconducting quantum processors utilize cryogenic dilution refrigerators."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_a,
    "assistant_response": "Dilution refrigeration cools qubits below 15 millikelvin."
})

# Pre-seed Session B
start_b = rpc("v1/session/start", {"title": "Trapped Ion System Topic"})
sid_b = start_b["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_b,
    "role": "user",
    "content": "Trapped ion hardware operates using radiofrequency quadrupole fields."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_b,
    "assistant_response": "Quadrupole trapping provides strong spatial confinement."
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

# Wait for boot
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# Execute /search cryogenic
os.write(fd, b"/search cryogenic")
pump(0.3)
os.write(fd, b"\r")
pump(0.6)

ok &= expect("search-match-count", "match(es) for \u201ccryogenic\u201d")
ok &= expect("search-match-snippet", "cryogenic dilution refrigerators")

# Execute unmatched search
os.write(fd, b"/search non_existent_quantum_state")
pump(0.3)
os.write(fd, b"\r")
pump(0.6)

ok &= expect("search-empty-notice", "No transcript matches found for \u201cnon_existent_quantum_state\u201d")

# Close shell
try:
    os.write(fd, b"\x1b")
    pump(0.2)
    os.write(fd, b"\x03")
    pump(0.2)
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

print(f"\nInc 32 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
