#!/usr/bin/env python3
"""Increment 36 PTY smoke: Context Provenance & Retrieval Observability.

Proves against a live daemon that:
1. Streamed generations faithfully capture and project context retrieval telemetry.
2. Augmented turns render a subtle context badge (✦ Context: ...).
3. /context overlay displays the exact retrieval epoch, candidate count, assembled memories, injected tokens, and latency.
4. /context can be dismissed cleanly with Esc/Enter.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc36-pty-")
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

# Pre-seed memory node into knowledge graph
rpc("v1/memory/store", {
    "label": "Cryogenic Transmon Cooling",
    "content": "Dilution refrigerators cool transmon qubits down to 15 millikelvin temperatures.",
    "scope": "global",
    "channel": "core"
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

def expect(label, needle, timeout=12.0):
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

# Submit a prompt that retrieves the seeded concept
send_text("How do transmon qubits cool down?")

# Check for turn completion and context badge
ok &= expect("user-prompt-echo", "How do transmon qubits cool down?")
ok &= expect("mock-response", "Mock response to: How do transmon qubits cool down?")
ok &= expect("context-badge", "✦ Context:")

pump(1.0)

# Open /context overlay
send_text("/context")
pump(0.8)

ok &= expect("context-modal-title", "Context Provenance")
ok &= expect("retrieval-epoch-field", "Retrieval Epoch:")
ok &= expect("memory-augmentation-header", "Memory Augmentation:")
ok &= expect("candidates-evaluated-field", "Candidates evaluated:")
ok &= expect("memories-assembled-field", "Memories assembled into prompt:")
ok &= expect("tokens-injected-field", "Context tokens injected:")
ok &= expect("latency-field", "Assembly latency:")

# Dismiss /context overlay
send_char("\x1b", 0.4)
pump(0.4)
ok &= expect("closed-context-notice", "Closed context provenance view")

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

print(f"\nInc 36 PTY smoke results: {'PASS' if ok else 'FAIL'}")
if not ok:
    print("--- TERMINAL BUFFER ---")
    print(clean(buf))
    print("-----------------------")
sys.exit(0 if ok else 1)
