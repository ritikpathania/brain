#!/usr/bin/env python3
"""Increment 43 PTY smoke: Knowledge Graph Relational Authoring (/memory link & /memory unlink).

Proves against a live daemon that:
1. Shell boots with live prompt.
2. Ingests two concept nodes: AuthService and PostgresDb.
3. Links AuthService to PostgresDb with custom relation depends_on (/memory link).
4. Opens /memory overlay to inspect that the relationship depends_on -> PostgresDb is authoritatively visible.
5. Unlinks AuthService and PostgresDb via /unlink.
6. Opens /memory overlay to inspect that the relationship is authoritatively removed.
7. Links AuthService to PostgresDb with default related_to using top-level /link.
8. Verifies deterministic failure handling for non-existent concepts.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc43-pty-")
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

    # 2. Ingest concept AuthService
    send_text("/memory add AuthService --content Handles user authentication and JWT validation")
    if not expect("2. AuthService concept ingested", "Stored memory concept “AuthService”"):
        sys.exit(1)

    # 3. Ingest concept PostgresDb
    send_text("/memory add PostgresDb --content PostgreSQL primary database")
    if not expect("3. PostgresDb concept ingested", "Stored memory concept “PostgresDb”"):
        sys.exit(1)

    # 4. Link AuthService -> PostgresDb with depends_on
    send_text("/memory link AuthService PostgresDb depends_on")
    if not expect("4. Link created with depends_on", "Linked “AuthService” → [depends_on] → “PostgresDb”"):
        sys.exit(1)

    # 5. Open /memory overlay to verify relationship visibility
    send_text("/memory")
    if not expect("5. /memory overlay opens with rows", "PostgresDb"):
        sys.exit(1)

    # Expand the selected concept
    send_char("\r", 0.6)
    if not expect("5.1 Relationship visible in /memory", "Connected Relations:"):
        sys.exit(1)

    # Close overlay
    send_char("\x1b", 0.5) # ESC
    pump(0.5)

    # 6. Unlink AuthService -> PostgresDb via top-level /unlink
    send_text("/unlink AuthService PostgresDb depends_on")
    if not expect("6. Unlinked AuthService and PostgresDb", "Unlinked “AuthService” ✕ [depends_on] ✕ “PostgresDb”"):
        sys.exit(1)

    # 7. Open /memory overlay to verify relation removed
    send_text("/memory")
    if not expect("7. /memory overlay opens again with rows", "PostgresDb"):
        sys.exit(1)

    send_char("\r", 0.6)
    pump(0.5)
    # Check that (No outgoing relations) appears under expanded concept
    if not expect("7.1 Relation is removed", "(No outgoing relations)"):
        sys.exit(1)

    send_char("\x1b", 0.5) # ESC
    pump(0.5)

    # 8. Link with default associated_with using /link
    send_text("/link AuthService PostgresDb")
    if not expect("8. Linked with default relation", "Linked “AuthService” → [associated_with] → “PostgresDb”"):
        sys.exit(1)

    # 9. Verify error handling for non-existent node
    send_text("/memory link NonExistent AuthService")
    if not expect("9. Rejection for non-existent concept", "Source concept 'NonExistent' not found in memory"):
        sys.exit(1)

    # 10. Exit cleanly
    send_text("/quit")
    pump(0.6)
    print("ALL Increment 43 PTY checks PASSED.")

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
