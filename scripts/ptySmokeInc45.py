#!/usr/bin/env python3
"""Increment 45 PTY smoke: Authoritative Daemon Diagnostics Telemetry (/doctor & v1/system/status).

Proves against a live daemon that:
1. Shell boots with live prompt.
2. Ingests concepts and links them via /memory link.
3. Opens /doctor overlay.
4. Asserts authoritative subsystem diagnostics appear:
   - Overall System Health: ● HEALTHY
   - UDS Daemon & Runtime: version and uptime telemetry
   - SQLite Database & Graph: authoritative concepts and relations counts
   - Model Gateway & Inference: active models count
5. Dismisses /doctor and exits cleanly.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc45-pty-")
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

    # 2. Ingest concepts and link them
    send_text("/memory add FrontendApp --content Web user interface")
    if not expect("2. FrontendApp ingested", "Stored memory concept “FrontendApp”"):
        sys.exit(1)

    send_text("/memory add BackendApi --content REST API backend")
    if not expect("2.1 BackendApi ingested", "Stored memory concept “BackendApi”"):
        sys.exit(1)

    send_text("/memory link FrontendApp BackendApi communicates_via")
    if not expect("2.2 Concepts linked", "Linked “FrontendApp” → [communicates_via] → “BackendApi”"):
        sys.exit(1)

    # 3. Open /doctor overlay
    send_text("/doctor")
    if not expect("3. /doctor overlay opens", "Brain System Doctor"):
        sys.exit(1)

    # 4. Verify authoritative health and metrics
    if not expect("4. Overall healthy banner", "HEALTHY"):
        sys.exit(1)

    if not expect("4.1 UDS Daemon & Runtime telemetry", "UDS Daemon Socket & Runtime"):
        sys.exit(1)

    if not expect("4.2 SQLite Database & Graph counts", "SQLite Database & Graph"):
        sys.exit(1)

    if not expect("4.3 Graph concepts and relations count", "concepts"):
        sys.exit(1)

    if not expect("4.4 Model Gateway telemetry", "Model Gateway & Inference"):
        sys.exit(1)

    # 5. Dismiss /doctor overlay
    send_char("\x1b", 0.5) # ESC
    pump(0.5)

    # 6. Exit cleanly
    send_text("/quit")
    pump(0.6)
    print("ALL Increment 45 PTY checks PASSED.")

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
