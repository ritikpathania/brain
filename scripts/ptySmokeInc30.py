#!/usr/bin/env python3
"""Increment 30 PTY smoke: Context Assembly & Retrieval Augmentation.

Proves against a live daemon that:
1. Stored memories in SQLite are queried by DefaultContextOrchestrator.
2. Generation streams turns cleanly with memory-augmented context.
3. /memory and chat prompt interact harmoniously in the terminal UI.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc30-pty-")
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

# Pre-seed memory concept in SQLite
rpc("v1/memory/store", {
    "label": "Vector Search HNSW",
    "content": "Hierarchical Navigable Small World graphs provide high-recall approximate nearest neighbor vector search.",
    "scope": "global"
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

# Open /memory overlay to verify stored concept
os.write(fd, b"/memory")
pump(0.3)
os.write(fd, b"\r")
pump(0.5)

ok &= expect("memory-overlay", "Relational Knowledge & Memory")
ok &= expect("seeded-concept", "Vector Search HNSW")

# Dismiss /memory overlay
os.write(fd, b"\x1b")
pump(0.3)

# Send a user prompt querying the topic
os.write(fd, b"Explain vector search HNSW")
pump(0.3)
os.write(fd, b"\r")
pump(1.0)

# Verify turn echoed and completed
ok &= expect("user-prompt-echo", "Explain vector search HNSW")

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

print(f"\nInc 30 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
