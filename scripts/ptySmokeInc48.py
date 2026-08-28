#!/usr/bin/env python3
"""Increment 48 PTY smoke: Persistent Per-Session System Prompt (SQLite Migration v17).

Proves against a live daemon that:
1. Shell boots with live prompt against SQLite storage with Migration v17 applied.
2. Configures custom system prompt via `/system You are an expert compiler engineer specializing in LLVM`.
3. Inspects `/session info` -> confirms custom instructions are displayed.
4. Generates a turn -> verifies completion and persistence.
5. Boots a fresh secondary shell process against the same daemon/DB, resumes the session.
6. Inspects `/session info` on resumed session -> confirms restored custom instructions.
7. Forks session via `/fork Forked LLVM Session` -> confirms forked session preserves instructions.
8. Clears instructions on forked session via `/system clear` -> confirms reset to default.
9. Resumes original session -> confirms original session instructions remain intact.
10. Exits cleanly.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc48-pty-")
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

# Helper to run a shell session
class ShellProcess:
    def __init__(self):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.environ["BRAIN_SOCKET_PATH"] = sock_path
            os.environ["TERM"] = "xterm-256color"
            os.chdir("/Users/ritikpathania/Developer/PyCharm/brain/packages/brain-shell")
            os.execvp("bun", ["bun", "run", "src/main.tsx"])
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        self.buf = b""

    def pump(self, seconds=0.4):
        end = time.time() + seconds
        while time.time() < end:
            r, _, _ = select.select([self.fd], [], [], 0.05)
            if self.fd in r:
                try:
                    chunk = os.read(self.fd, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                self.buf += chunk

    def send_char(self, ch, delay=0.08):
        os.write(self.fd, ch.encode("utf-8") if isinstance(ch, str) else ch)
        self.pump(delay)

    def send_text(self, text, delay=0.05):
        for ch in text:
            self.send_char(ch, delay)
        self.send_char("\r", 0.4)

    def expect(self, label, needle, timeout=12.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.pump(0.1)
            if needle in clean(self.buf):
                print("PASS " + label)
                return True
        print("FAIL %s: %r not seen" % (label, needle))
        print("CURRENT BUFFER:\n" + clean(self.buf))
        return False

    def close(self):
        try:
            os.kill(self.pid, signal.SIGKILL)
            os.waitpid(self.pid, 0)
        except Exception:
            pass

try:
    # 1. First Shell Session
    sh1 = ShellProcess()
    sh1.pump(1.2)
    if not sh1.expect("1. Shell 1 boots with prompt", "brain-default"):
        sys.exit(1)

    # 2. Configure custom system prompt
    prompt_text = "You are an expert compiler engineer specializing in LLVM"
    sh1.send_text("/system " + prompt_text)
    if not sh1.expect("2. System prompt configured", "Set system instructions: “You are an expert compiler engineer specializing in LLVM”"):
        sys.exit(1)

    # 3. Check /session info
    sh1.send_text("/session info")
    if not sh1.expect("3. Session info displays custom instructions", "System Instructions: “You are an expert compiler engineer specializing in LLVM”"):
        sys.exit(1)

    # 4. Generate turn
    sh1.send_text("Explain LLVM Intermediate Representation")
    if not sh1.expect("4. Turn completed", "brain-shell · model"):
        sys.exit(1)

    # Capture session ID from session info
    sh1.send_text("/session info")
    sh1.pump(0.6)
    buf_text = clean(sh1.buf)
    matches = [m for m in re.findall(r"Session ID:\s+([0-9a-zA-Z_-]+)", buf_text) if m != "ephemeral"]
    session_id = matches[-1] if matches else None
    print(f"Captured Session ID: {session_id}")
    if not session_id:
        print("FAIL: Could not capture session ID from session info")
        sys.exit(1)

    sh1.send_text("/quit")
    sh1.pump(0.6)
    sh1.close()

    # 5. Second Shell Session (Resume across process boundary)
    sh2 = ShellProcess()
    sh2.pump(1.2)
    if not sh2.expect("5. Shell 2 boots fresh", "brain-default"):
        sys.exit(1)

    # Resume the session
    sh2.send_text(f"/resume {session_id}")
    if not sh2.expect("5.1 Session resumed", f"Resumed “"):
        sys.exit(1)

    # 6. Verify restored custom prompt in /session info
    sh2.send_text("/session info")
    if not sh2.expect("6. Restored prompt in session info", "System Instructions: “You are an expert compiler engineer specializing in LLVM”"):
        sys.exit(1)

    # 7. Fork session
    sh2.send_text("/fork Forked LLVM Session")
    if not sh2.expect("7. Session forked", "Forked to “Forked LLVM Session”"):
        sys.exit(1)

    sh2.send_text("/session info")
    if not sh2.expect("7.1 Forked session inherits prompt", "System Instructions: “You are an expert compiler engineer specializing in LLVM”"):
        sys.exit(1)

    # 8. Clear prompt on forked session
    sh2.send_text("/system clear")
    if not sh2.expect("8. Fork prompt cleared", "Cleared custom system instructions (using default engine prompt)."):
        sys.exit(1)

    sh2.send_text("/session info")
    if not sh2.expect("8.1 Forked session has default prompt", "System Instructions: default engine prompt (no custom instructions)"):
        sys.exit(1)

    # 9. Resume original session again
    sh2.send_text(f"/resume {session_id}")
    if not sh2.expect("9. Resumed original session", f"Resumed “"):
        sys.exit(1)

    sh2.send_text("/session info")
    if not sh2.expect("9.1 Original session retained prompt", "System Instructions: “You are an expert compiler engineer specializing in LLVM”"):
        sys.exit(1)

    # 10. Exit cleanly
    sh2.send_text("/quit")
    sh2.pump(0.6)
    sh2.close()
    print("ALL Increment 48 PTY checks PASSED.")

finally:
    try:
        os.kill(daemon_proc.pid, signal.SIGTERM)
        daemon_proc.wait(timeout=2.0)
    except Exception:
        pass
    import shutil
    shutil.rmtree(temp_dir, ignore_errors=True)
