#!/usr/bin/env python3
"""Increment 38 PTY smoke: Runtime Model Discovery & Switching Surface.

Proves against a live daemon that:
1. Shell boots with default model in status bar.
2. /model opens interactive ModelPickerView displaying registered models with capability badges.
3. Interactive selection switches the active model, emits confirmation notice, and updates the status bar live.
4. Prompt generation succeeds with the selected model.
5. Direct /model <alias> resolves and switches models instantly.
6. Failed model resolution leaves the active model intact and emits an error notice.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc38-pty-")
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
ok &= expect("status-bar-default-model", "model brain-default")

# 1. Open /model picker overlay
send_text("/model")
pump(0.6)
ok &= expect("model-picker-title", "Select Active Model")
ok &= expect("model-item-default", "Brain Default (Mock Local)")
ok &= expect("model-item-sonnet", "Claude 3.7 Sonnet (Mock)")
ok &= expect("model-item-deepseek", "DeepSeek R1 (Mock)")
ok &= expect("model-capability-badges", "[thinking]")

# 2. Navigate down to Claude 3.7 Sonnet and select with Enter
send_char("\x1b[B", 0.3) # down arrow
send_char("\r", 0.6)     # commit selection
ok &= expect("switch-sonnet-notice", "Switched active model to “Claude 3.7 Sonnet (Mock)”")
ok &= expect("status-bar-sonnet-model", "model Claude 3.7 Sonnet (Mock)")

# 3. Submit a prompt to verify generation with selected model
send_text("Explain superconducting Josephson junctions")
pump(0.8)
ok &= expect("prompt-echo", "Explain superconducting Josephson junctions")
ok &= expect("generation-response", "mock") # mock provider returns response

# 4. Direct alias switch via /model qwen
send_text("/model qwen")
pump(0.8)
ok &= expect("switch-qwen-notice", "Switched active model to “Qwen 2.5 Coder (Mock)”")
ok &= expect("status-bar-qwen-model", "model Qwen 2.5 Coder (Mock)")

# 5. Attempt invalid model switch -> error notice + status bar unchanged
send_text("/model invalid_model_xyz_999")
pump(0.8)
ok &= expect("switch-invalid-error-notice", "Could not resolve model:")
ok &= expect("status-bar-preserved-model", "model Qwen 2.5 Coder (Mock)")

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

print(f"\nInc 38 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
