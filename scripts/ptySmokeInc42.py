#!/usr/bin/env python3
"""Increment 42 PTY smoke: Model Reasoning Guarding & Thinking Status Observability.

Proves against a live daemon that:
1. Shell boots with default thinking state think:off in status line.
2. /thinking 4096 updates status line to think:4k.
3. Switching to non-thinking model (/model qwen2.5-coder:32b):
   - Emits non-thinking capability notice.
   - Status line updates to think:n/a.
4. Generation turn on non-thinking model succeeds with guarded budget.
5. Switching back to brain-default restores configured think:4k.
6. /thinking off returns status line to think:off.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc42-pty-")
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
ok &= expect("default-status-think-off", "think:off")

# 1. Set budget to 4096 -> status line think:4k
send_text("/thinking 4096")
pump(0.6)
ok &= expect("set-budget-notice", "Reasoning thinking budget set to 4096 tokens.")
ok &= expect("status-think-4k", "think:4k")

# 2. Switch to non-thinking model qwen2.5-coder:32b
send_text("/model qwen")
pump(0.6)
ok &= expect("model-switch-notice", "Switched active model to “Qwen 2.5 Coder (Mock)” (qwen2.5-coder:32b)")
ok &= expect("non-thinking-warning", "does not support reasoning thinking")
ok &= expect("status-think-na", "think:n/a")

# 3. Submit prompt on non-thinking model
send_text("Write a quick python function")
pump(1.2)
ok &= expect("prompt-echo", "Write a quick python function")
ok &= expect("generation-reply", "mock")

# 4. Switch back to reasoning model brain-default
send_text("/model brain-default")
pump(0.6)
ok &= expect("restore-model-notice", "Switched active model to “Brain Default (Mock Local)” (brain-default)")
ok &= expect("status-think-4k-restored", "think:4k")

# 5. Disable thinking budget
send_text("/thinking off")
pump(0.6)
ok &= expect("disable-budget-notice", "Reasoning thinking budget disabled.")
ok &= expect("status-think-off-restored", "think:off")

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

print(f"\nInc 42 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
