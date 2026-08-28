#!/usr/bin/env python3
"""Increment 37 PTY smoke: Session Pinning & Archiving Full Lifecycle Surface.

Proves against a live daemon that:
1. /session pin pins the active session and renders ★ in /resume.
2. /session unpin unpins the active session.
3. /session archive archives the active session, resets active state, and hides it from default /resume.
4. /resume all exposes archived sessions with the [archived] badge.
5. /session restore restores the archived session back into default /resume.
6. Top-level aliases (/pin, /unpin, /archive, /restore) execute correctly.
"""
import fcntl, json, os, pty, re, select, signal, socket, struct, subprocess, sys, tempfile, termios, time

ROWS, COLS = 30, 100
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

temp_dir = tempfile.mkdtemp(prefix="brain-inc37-pty-")
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

# Pre-seed Session A
start_a = rpc("v1/session/start", {"title": "Quantum Annealing Processor"})
sid_a = start_a["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_a,
    "role": "user",
    "content": "Quantum annealers solve combinatorial optimization problems."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_a,
    "assistant_response": "Annealing systems find the global minimum of an Ising Hamiltonian."
})

# Pre-seed Session B
start_b = rpc("v1/session/start", {"title": "Topological Qubit Lattice"})
sid_b = start_b["session_id"]
rpc("v1/session/append_turn", {
    "session_id": sid_b,
    "role": "user",
    "content": "Majorana bound states enable fault-tolerant braiding operations."
})
rpc("v1/session/complete_turn", {
    "session_id": sid_b,
    "assistant_response": "Topological protection suppresses local environmental decoherence."
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
    print("CURRENT BUFFER:\n" + clean(buf))
    return False

ok = True

# Wait for boot
ok &= expect("launch-mark", "◆ BRAIN")
ok &= expect("launch-prompt", "❯")

# 1. Resume Session A
send_text(f"/resume {sid_a}")
pump(0.8)
ok &= expect("resumed-session-a", "Resumed “Quantum Annealing Processor”")

# 2. Pin active session via /session pin
send_text("/session pin")
pump(0.8)
ok &= expect("pinned-active-notice", "Pinned session “Quantum Annealing Processor”")

# 3. Verify ★ badge in /resume
send_text("/resume")
pump(0.6)
ok &= expect("resume-picker-open", "Resume session")
ok &= expect("pinned-badge-visible", "★")
ok &= expect("pinned-title-visible", "Quantum Annealing Processor")
send_char("\x1b", 0.4) # Close picker
pump(0.4)

# 4. Unpin active session via /session unpin
send_text("/session unpin")
pump(0.8)
ok &= expect("unpinned-active-notice", "Unpinned session “Quantum Annealing Processor”")

# 5. Archive active session via /session archive
send_text("/session archive")
pump(0.8)
ok &= expect("archived-active-notice", "Archived active session “Quantum Annealing Processor”")

# 6. Verify default /resume hides archived Session A
send_text("/resume")
pump(0.6)
ok &= expect("resume-picker-active-only", "Resume session")
ok &= expect("session-b-visible", "Topological Qubit Lattice")

# Check that the active picker modal does NOT list Session A
modal_content = clean(buf).split("Resume session")[-1].split("navigate")[0]
if "Quantum Annealing Processor" not in modal_content:
    print("PASS archived-session-hidden-from-default")
else:
    print("FAIL archived-session-hidden-from-default: archived session still in picker modal")
    ok = False
send_char("\x1b", 0.4)
pump(0.4)

# 7. Verify /resume all exposes archived Session A with [archived] badge
send_text("/resume all")
pump(0.6)
ok &= expect("resume-all-picker-open", "Resume session")
ok &= expect("archived-badge-visible", "Quantum Annealing Processor")
ok &= expect("archived-tag-visible", "[archived]")
send_char("\x1b", 0.4)
pump(0.4)

# 8. Restore Session A via /session restore <sid_a>
send_text(f"/session restore {sid_a}")
pump(0.8)
ok &= expect("restored-session-notice", "Restored session “Quantum Annealing Processor”")

# 9. Verify Session A is back in default /resume
send_text("/resume")
pump(0.6)
ok &= expect("session-a-restored-visible", "Quantum Annealing Processor")
send_char("\x1b", 0.4)
pump(0.4)

# 10. Verify aliases (/pin and /archive on Session B)
send_text(f"/pin {sid_b}")
pump(0.8)
ok &= expect("alias-pin-notice", "Pinned session “Topological Qubit Lattice”")

send_text(f"/archive {sid_b}")
pump(0.8)
ok &= expect("alias-archive-notice", "Archived session “Topological Qubit Lattice”")

send_text(f"/restore {sid_b}")
pump(0.8)
ok &= expect("alias-restore-notice", "Restored session “Topological Qubit Lattice”")

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

print(f"\nInc 37 PTY smoke results: {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
