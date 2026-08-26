#!/usr/bin/env python3
"""Increment 21 PTY smoke: /doctor + /memory wired through the canonical
registry, against a REAL daemon.

Flows:
  A. Type /doctor -> diagnostics modal appears with HEALTHY banner -> enter
     dismisses with the system notice.
  B. Seed one memory via RPC (with a relation), type /memory -> modal opens,
     type "cortex" to filter -> seeded node lists, enter expands the detail
     pane showing the stored relation target ("Beta Concept"), esc closes
     with the system notice.
"""
import fcntl, json, os, pty, re, select, shutil, signal, socket, struct, subprocess, sys, termios, time, uuid

ROWS, COLS = 30, 100
REPO = "/Users/ritikpathania/Developer/PyCharm/brain"
PKG_DIR = f"{REPO}/packages/brain-shell"
TMP = "/tmp/brain-inc21-smoke"
SOCK = f"{TMP}/brain.sock"
CONFIG_FILE = f"{TMP}/config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

LABEL = "Alpha Cortex Node"
CONTENT = "Cortex excerpt body for the smoke"

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

def rpc(action, body, timeout=10.0):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(timeout)
    s.connect(SOCK)
    fobj = s.makefile("rw")
    req = {"version": "1.0", "type": "Request", "id": f"smoke-{uuid.uuid4().hex[:8]}",
           "action": action, "body": json.dumps(body)}
    fobj.write(json.dumps(req) + "\n"); fobj.flush()
    resp = json.loads(fobj.readline())
    s.close()
    raw = resp["body"]
    return json.loads(raw) if isinstance(raw, str) else raw

seeded = rpc("v1/memory/store", {
    "label": LABEL,
    "content": CONTENT,
    "scope": "workspace",
    "relations": [{"relation": "supports", "target_id": "beta-1", "target_label": "Beta Concept"}],
})
assert seeded.get("success") is not False, f"memory/store failed: {seeded}"

failures = []
def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        failures.append(name)

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

def run_slash(name):
    send_key("/")
    for ch in name:
        send_key(ch, 0.15)
    send_key("\r")
    pump(0.6)

# ── Flow A: /doctor ───────────────────────────────────────────────────────
run_slash("doctor")
check("A1 doctor modal opens", wait_for("Brain System Doctor"))
check("A2 healthy banner", wait_for("HEALTHY"))
check("A3 subsystem probes listed", wait_for("UDS Daemon Socket"))
send_key("\r")
check("A4 dismissed with notice", wait_for("Completed system diagnostics", count=1))

# ── Flow B: /memory ───────────────────────────────────────────────────────
# The overlay's initial empty-query fetch is deliberately not asserted:
# server-side behavior for query:'' is unspecified. Instead we type a token
# the sole seeded node contains ("cortex") — the private tmp DB guarantees
# it ranks first — and prove listing + expansion behaviorally.
run_slash("memory")
check("B1 memory modal opens", wait_for("Relational Knowledge & Memory"))
for ch in "cortex":
    send_key(ch, 0.3)   # each keystroke re-fires the 200ms debounced search
pump(0.8)
check("B2 filtered listing shows the seeded concept", wait_for(LABEL))
send_key("\r")
# Detail pane opens; the daemon recovers stored relations at the
# memory/search boundary, so the expanded pane renders the seeded edge target.
check("B3 expand renders the stored relation target", wait_for("Beta Concept", timeout=15))
send_key("\x1b")
pump(0.4)
check("B4 dismissed with notice", wait_for("Closed memory exploration view"))

# ── Teardown ──────────────────────────────────────────────────────────────
os.write(fd, b"\x03")
pump(0.5)
try:
    os.kill(pid, signal.SIGTERM)
except ProcessLookupError:
    pass
try:
    daemon.terminate()
    daemon.wait(timeout=10)
except subprocess.TimeoutExpired:
    daemon.kill()

print("FAILURES:", len(failures))
sys.exit(1 if failures else 0)
