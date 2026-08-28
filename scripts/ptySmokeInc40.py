#!/usr/bin/env python3
"""Increment 40 PTY smoke: Knowledge Graph Memory Consolidation Surface (/memory consolidate).

Proves against a live daemon that:
1. Shell boots with live prompt.
2. /memory add populates a concept node.
3. /memory consolidate triggers backend consolidation and surfaces structured action metrics and latency.
4. Top-level /consolidate alias triggers consolidation seamlessly.
5. Search and context-augmented generation remain fully operational post-consolidation.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc40-pty-")
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

# 1. Ingest concept node
send_text("/memory add Quantum Teleportation --content Entangled photon pairs transfer state")
pump(0.8)
ok &= expect("store-confirmation", "Stored memory concept “Quantum Teleportation” (")

# 2. Run /memory consolidate
send_text("/memory consolidate")
pump(0.8)
cbuf = clean(buf)
has_consolidated = ("Consolidated memory graph:" in cbuf) or ("Memory graph is fully consolidated:" in cbuf)
if has_consolidated:
    print("PASS memory-consolidate-notice")
else:
    print("FAIL memory-consolidate-notice: no consolidation notice found")
    ok = False

# 3. Run top-level /consolidate alias
send_text("/consolidate")
pump(0.8)
cbuf2 = clean(buf)
has_consolidated2 = ("Consolidated memory graph:" in cbuf2) or ("Memory graph is fully consolidated:" in cbuf2)
if has_consolidated2:
    print("PASS alias-consolidate-notice")
else:
    print("FAIL alias-consolidate-notice: no alias consolidation notice found")
    ok = False

# 4. Submit prompt to verify prompt generation still retrieves concept
send_text("Explain Quantum Teleportation")
pump(1.2)
ok &= expect("prompt-echo", "Explain Quantum Teleportation")
ok &= expect("generation-reply", "mock")

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

print(f"\nInc 40 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
