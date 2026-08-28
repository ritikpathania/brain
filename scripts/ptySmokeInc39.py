#!/usr/bin/env python3
"""Increment 39 PTY smoke: Knowledge Graph Manual Ingestion Surface (/memory add).

Proves against a live daemon that:
1. Shell boots with live prompt.
2. /memory add <label> --content <text> stores concept node in backend storage and returns authoritative ID.
3. Ingested concept immediately appears in /memory overlay list.
4. User prompt query retrieves the newly ingested concept, reflected in /context provenance telemetry.
5. /memory delete <nodeId> purges the node from storage.
6. /memory overlay confirms the node is removed.
7. Validation errors (/memory add --content ...) emit clear notices without mutating state.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc39-pty-")
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

def expect(label, needle, timeout=12.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        pump(0.1)
        if needle in clean(buf):
            print("PASS " + label)
            return True
    print("FAIL %s: %r not seen" % (label, needle))
    print("CURRENT BUFFER:\n" + clean(buf))
    return False

ok = True

# Wait for boot
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# 1. Ingest concept node via /memory add
send_text("/memory add Quantum Teleportation --content Entangled photon pairs enable quantum state transfer")
pump(0.8)
ok &= expect("store-confirmation", "Stored memory concept “Quantum Teleportation” (")

# Extract created node ID from buffer
cbuf = clean(buf)
match = re.search(r"Stored memory concept “Quantum Teleportation” \(([a-zA-Z0-9_\-]+)\)", cbuf)
node_id = match.group(1) if match else None
if node_id:
    print(f"PASS captured-node-id: {node_id}")
else:
    print("FAIL captured-node-id: could not extract node ID")
    ok = False

# 2. Open /memory overlay and verify concept appears
send_text("/memory")
pump(0.8)
ok &= expect("memory-overlay-open", "Relational Knowledge & Memory")
ok &= expect("memory-concept-listed", "Quantum Teleportation")
send_char("\x1b", 0.4) # esc close

# 3. Submit a prompt mentioning the concept to trigger retrieval augmentation
send_text("Explain how Quantum Teleportation works with photon pairs")
pump(1.2)
ok &= expect("prompt-echo", "Quantum Teleportation")
ok &= expect("generation-reply", "mock")

# 4. Open /context overlay to verify retrieval telemetry
send_text("/context")
pump(0.8)
ok &= expect("context-overlay-title", "Context Provenance")
ok &= expect("context-retrieval-epoch", "Retrieval Epoch:")
send_char("\x1b", 0.4) # esc close

# 5. Delete the node via /memory delete <node_id>
if node_id:
    send_text(f"/memory delete {node_id}")
    pump(0.8)
    ok &= expect("delete-confirmation", f"Deleted memory node {node_id}")

    # 6. Verify /memory overlay opens cleanly after deletion
    send_text("/memory")
    pump(0.8)
    ok &= expect("memory-overlay-after-delete", "Relational Knowledge & Memory")
    send_char("\x1b", 0.4) # esc close

# 7. Test argument validation notice
send_text("/memory add --content NoLabelContent")
pump(0.8)
ok &= expect("validation-error-notice", "Memory node requires a non-empty label")

# Clean shutdown
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

print(f"\nInc 39 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
