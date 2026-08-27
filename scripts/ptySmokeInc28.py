#!/usr/bin/env python3
"""Increment 28 PTY smoke: Canonical Graph Edge Materialization.

Proves against a live daemon that:
1. Canonical graph edges persisted in SQLite edges table are materialized.
2. Expanding a concept in /memory renders 'Connected Relations:' with the resolved neighbor label.
"""
import fcntl, json, os, pty, re, select, signal, socket, sqlite3, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc28-pty-")
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

# Wait for socket
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

# 2. Seed memory concepts via UDS RPC
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
    return json.loads(line)

res_a = rpc("v1/memory/store", {
    "label": "Core Architecture Node",
    "content": "Core architecture subsystem",
    "scope": "workspace"
})
body_a = res_a["body"] if isinstance(res_a["body"], dict) else json.loads(res_a["body"])
node_a_id = body_a["node_id"]

res_b = rpc("v1/memory/store", {
    "label": "Event Dispatch Engine",
    "content": "Asynchronous event engine",
    "scope": "workspace"
})
body_b = res_b["body"] if isinstance(res_b["body"], dict) else json.loads(res_b["body"])
node_b_id = body_b["node_id"]

# 3. Insert canonical edge into the SQLite database directly
conn = sqlite3.connect(db_path)
conn.execute(
    "INSERT INTO edges (source, target, relation, weight, updated_at) VALUES (?, ?, ?, ?, ?)",
    (node_a_id, node_b_id, "depends_on", 1.0, int(time.time()))
)
conn.commit()
conn.close()

# 4. Launch brain-shell in PTY
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

# Open /memory with query "Core"
os.write(fd, b"/memory Core")
pump(0.3)
os.write(fd, b"\r")
pump(0.5)

# Verify modal open and Concept A listed
ok &= expect("memory-modal", "Relational Knowledge & Memory")
ok &= expect("core-concept", "Core Architecture Node")

# Press Enter to expand the selected row
os.write(fd, b"\r")
pump(0.5)

# Verify expanded view shows Connected Relations with resolved neighbor label
ok &= expect("connected-relations-header", "Connected Relations:")
ok &= expect("relation-resolved-label", "depends_on → Event Dispatch Engine")

# Close modal
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

print(f"\nInc 28 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
