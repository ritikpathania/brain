#!/usr/bin/env python3
"""Increment 47 PTY smoke: Retrieval Context Budget Steerability (/context budget).

Proves against a live daemon that:
1. Shell boots with live prompt.
2. Runs /context budget -> reports engine default (2048 tokens).
3. Sets /context budget 4096 -> reports 4096 tokens confirmation.
4. Runs /session info -> asserts 'Context Budget: 4096 tokens (configured)'.
5. Ingests concept via /memory add and generates a turn -> asserts completion.
6. Runs /context budget default -> reports reset to engine default (2048 tokens).
7. Runs /session stats -> asserts 'Context Budget: 2048 tokens (engine default)'.
8. Exits cleanly.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc47-pty-")
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

try:
    # 1. Shell boots
    pump(1.2)
    if not expect("1. Shell boots with prompt", "brain-default"):
        sys.exit(1)

    # 2. Check default context budget
    send_text("/context budget")
    if not expect("2. Default context budget reported", "Current context retrieval budget: 2048 tokens (engine default)."):
        sys.exit(1)

    # 3. Configure context budget
    send_text("/context budget 4096")
    if not expect("3. Context budget set to 4096", "Context retrieval budget set to 4096 tokens."):
        sys.exit(1)

    # 4. Check /session info reflects configured budget
    send_text("/session info")
    if not expect("4. Configured budget in session info", "Context Budget: 4096 tokens (configured)"):
        sys.exit(1)

    # 5. Ingest memory and execute turn
    send_text("/memory add Inc47Architecture Steering retrieval token budgets")
    if not expect("5. Memory added", "Stored memory concept “Inc47Architecture”"):
        sys.exit(1)

    send_text("Explain Inc47Architecture")
    if not expect("5.1 Generation completed", "brain-shell · model"):
        sys.exit(1)

    # 6. Reset context budget to default
    send_text("/context budget default")
    if not expect("6. Context budget reset", "Context retrieval budget reset to engine default (2048 tokens)."):
        sys.exit(1)

    # 7. Check /session stats reflects engine default
    send_text("/session stats")
    if not expect("7. Default budget in session stats", "Context Budget: 2048 tokens (engine default)"):
        sys.exit(1)

    # 8. Exit cleanly
    send_text("/quit")
    pump(0.6)
    print("ALL Increment 47 PTY checks PASSED.")

finally:
    try:
        os.kill(pid, signal.SIGTERM)
    except Exception:
        pass
    try:
        daemon_proc.terminate()
        daemon_proc.wait(timeout=2)
    except Exception:
        daemon_proc.kill()
    import shutil
    shutil.rmtree(temp_dir, ignore_errors=True)
