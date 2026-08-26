#!/usr/bin/env python3
"""Increment 24 PTY smoke: turn completion intact, idle esc harmless,
ctrl+c and /quit exit cleanly against a real daemon.

Flows:
  A. Normal turn completion with scripted mock reply -> composer idle again.
  B. Idle Escape is harmless -> app stays alive.
  C. Ctrl+C in idle state exits the shell with EOF.
  D. Fresh shell re-forked: /quit exits cleanly with EOF.
"""
import fcntl, json, os, pty, re, select, shutil, signal, socket, struct, subprocess, sys, termios, time, uuid

ROWS, COLS = 30, 100
REPO = "/Users/ritikpathania/Developer/PyCharm/brain"
PKG_DIR = f"{REPO}/packages/brain-shell"
TMP = "/tmp/brain-inc24-smoke"
SOCK = f"{TMP}/brain.sock"
CONFIG_FILE = f"{TMP}/config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

shutil.rmtree(TMP, ignore_errors=True)
os.makedirs(TMP, exist_ok=True)
with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto"}, f)

env = dict(os.environ)
env.update({
    "BRAIN_SOCKET_PATH": SOCK,
    "BRAIN_PID_PATH": f"{TMP}/brain.pid",
    "BRAIN_DB_PATH": f"{TMP}/brain.db",
    "BRAIN_ANALYTICS_DB_PATH": f"{TMP}/analytics.db",
    "BRAIN_CONFIG_DIR": TMP,
    "BRAIN_HEALTH_PORT": "0",
    "BRAIN_MOCK_SCRIPTED_RESPONSES": json.dumps(
        [{"tokens": ["Smoke reply one."], "finish_reason": "end_turn"}]
    ),
})
daemon = subprocess.Popen(
    ["target/debug/brain-daemon", "daemon", "run"], cwd=REPO, env=env,
    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
)
deadline = time.time() + 30
while time.time() < deadline:
    if os.path.exists(SOCK):
        try:
            probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            probe.connect(SOCK)
            probe.close()
            break
        except OSError:
            pass
    time.sleep(0.2)
else:
    sys.exit("FAIL: daemon never bound the socket")

failures = []
def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        failures.append(name)

# ── Session 1: Flows A, B, C ───────────────────────────────────────────────
pid, fd = pty.fork()
if pid == 0:
    os.chdir(PKG_DIR)
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["NODE_ENV"] = "production"
    os.execvp("bun", ["bun", "run", "src/main.tsx"])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf = bytearray()
def pump(seconds=0.4):
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd], [], [], 0.05)
        if r:
            try:
                chunk = os.read(fd, 65536)
                if not chunk:
                    return
                buf.extend(chunk)
            except OSError:
                return

def send_key(ch, delay=0.35):
    os.write(fd, ch.encode())
    pump(delay)

def wait_for(needle, timeout=25.0, count=1):
    end = time.time() + timeout
    while time.time() < end:
        if clean(bytes(buf)).count(needle) >= count:
            return True
        pump(0.2)
    return False

check("boot banner", wait_for("memory-first agent workspace", timeout=40))

# ── Flow A: Normal turn completion ─────────────────────────────────────────
send_key("h"); send_key("i"); send_key("\r")
check("A1 user turn echoed", wait_for("hi"))
check("A2 scripted assistant reply lands", wait_for("Smoke reply one.", timeout=20))
check("A3 composer idle again", wait_for("❯", count=2))

# ── Flow B: Idle Escape harmless ───────────────────────────────────────────
send_key("\x1b")
pump(0.5)
check("B1 app alive after idle esc", wait_for("memory-first agent workspace"))

# ── Flow C: Ctrl+C exits ───────────────────────────────────────────────────
os.write(fd, b"\x03")
eof_seen = False
end_c = time.time() + 10.0
while time.time() < end_c:
    r, _, _ = select.select([fd], [], [], 0.1)
    if r:
        try:
            chunk = os.read(fd, 65536)
            if not chunk:
                eof_seen = True
                break
            buf.extend(chunk)
        except OSError:
            eof_seen = True
            break
check("C1 ctrl+c exits the shell", eof_seen)
try:
    os.close(fd)
except OSError:
    pass
try:
    os.waitpid(pid, os.WNOHANG)
except ChildProcessError:
    pass

# ── Session 2: Flow D (/quit) ──────────────────────────────────────────────
pid2, fd2 = pty.fork()
if pid2 == 0:
    os.chdir(PKG_DIR)
    os.environ["BRAIN_SOCKET_PATH"] = SOCK
    os.environ["NODE_ENV"] = "production"
    os.execvp("bun", ["bun", "run", "src/main.tsx"])
fcntl.ioctl(fd2, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))

buf2 = bytearray()
def pump2(seconds=0.4):
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([fd2], [], [], 0.05)
        if r:
            try:
                chunk = os.read(fd2, 65536)
                if not chunk:
                    return
                buf2.extend(chunk)
            except OSError:
                return

def send_key2(ch, delay=0.35):
    os.write(fd2, ch.encode())
    pump2(delay)

def wait_for2(needle, timeout=25.0, count=1):
    end = time.time() + timeout
    while time.time() < end:
        if clean(bytes(buf2)).count(needle) >= count:
            return True
        pump2(0.2)
    return False

def run_slash2(name):
    send_key2("/")
    for ch in name:
        send_key2(ch, 0.15)
    send_key2("\r")
    pump2(0.6)

check("D0 boot banner 2", wait_for2("memory-first agent workspace", timeout=40))
run_slash2("quit")
quit_eof_seen = False
end_d = time.time() + 10.0
while time.time() < end_d:
    r, _, _ = select.select([fd2], [], [], 0.1)
    if r:
        try:
            chunk = os.read(fd2, 65536)
            if not chunk:
                quit_eof_seen = True
                break
            buf2.extend(chunk)
        except OSError:
            quit_eof_seen = True
            break
check("D1 /quit exits cleanly", quit_eof_seen)
try:
    os.close(fd2)
except OSError:
    pass
try:
    os.waitpid(pid2, os.WNOHANG)
except ChildProcessError:
    pass

# ── Teardown ──────────────────────────────────────────────────────────────
try:
    daemon.terminate()
    daemon.wait(timeout=10)
except subprocess.TimeoutExpired:
    daemon.kill()

print("FAILURES:", len(failures))
sys.exit(1 if failures else 0)
