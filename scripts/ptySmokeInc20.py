#!/usr/bin/env python3
"""Increment 20 PTY smoke: /resume fuzzy search + auto-titles against a REAL daemon.

Flows:
  A. Seed two titled sessions via RPC ("Alpha Groove Notes", "Beta Ledger"),
     open /resume by typing it into the composer, type-away a fuzzy fragment
     ("agrv") -> only Alpha remains, enter -> resumed transcript replays.
  B. Marker: after adopting a session, reopen /resume -> the ● sits on the
     live session's row.
  C. Auto-title: an UNTITLED session driven through one turn via RPC shows
     its derived prompt title in the picker, and that title is searchable.
"""
import fcntl, json, os, pty, re, select, shutil, signal, socket, struct, subprocess, sys, termios, time, uuid

ROWS, COLS = 30, 100
REPO = "/Users/ritikpathania/Developer/PyCharm/brain"
PKG_DIR = f"{REPO}/packages/brain-shell"
TMP = "/tmp/brain-inc20-smoke"
SOCK = f"{TMP}/brain.sock"
CONFIG_FILE = f"{TMP}/config.json"
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[=>]")

ALPHA_TITLE = "Alpha Groove Notes"
BETA_TITLE = "Beta Ledger"

def clean(buf: bytes) -> str:
    return ANSI.sub("", buf.decode("utf-8", "replace"))

shutil.rmtree(TMP, ignore_errors=True)
os.makedirs(TMP, exist_ok=True)
with open(CONFIG_FILE, "w") as f:
    json.dump({"theme": "auto"}, f)

# ── Real daemon on a private socket/db ────────────────────────────────────
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

def stream_one_turn(sid, prompt):
    """One generation turn over the raw-frame envelope; drains to finished."""
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(15.0)
    s.connect(SOCK)
    fobj = s.makefile("rw")
    gen = {"id": f"gen-{uuid.uuid4().hex[:8]}", "action": "v1/generation/stream",
           "payload": {"sessionId": sid,
                       "messages": [{"role": "user", "content": prompt}],
                       "model": "brain-default"}}
    fobj.write(json.dumps(gen) + "\n"); fobj.flush()
    for line in fobj:
        fr = json.loads(line)
        if fr.get("type") in ("finished", "error"):
            break
    s.close()

def seed(title):
    body = rpc("v1/session/create", {"title": title})
    sid = body["session_id"]
    # One turn so updatedAt ordering and replayable content exist.
    stream_one_turn(sid, f"seed turn for {title}")
    return sid

ALPHA_SID = seed(ALPHA_TITLE)
BETA_SID = seed(BETA_TITLE)

# Untitled session + one turn through the SAME daemon: the auto-title rule
# must fire server-side and rename it before any picker is opened.
UNTITLED_SID = rpc("v1/session/create", {})["session_id"]
stream_one_turn(UNTITLED_SID, "Plan the quarterly offsite")

failures = []
def check(name, cond):
    print(("PASS " if cond else "FAIL ") + name)
    if not cond:
        failures.append(name)

# ── Shell under PTY ───────────────────────────────────────────────────────
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

def open_resume():
    # '/' opens the inline slash-command palette; further chars narrow it;
    # return PASSES THROUGH the palette and submits the composer text.
    send_key("/")
    for ch in "resume":
        send_key(ch, 0.15)
    send_key("\r")
    pump(0.6)

# ── Flow A: type-away filter ──────────────────────────────────────────────
open_resume()
check("A2 picker opens", wait_for("Resume session"))
check("A3 both rows visible", wait_for(ALPHA_TITLE) and BETA_TITLE in clean(bytes(buf)))
for ch in "agrv":
    send_key(ch, 0.3)
# Judge only the tail AFTER the last query-line repaint: stale frames still
# contain both titles, so whole-buffer counting would false-pass/fail.
tail = clean(bytes(buf)).rsplit("agrv", 1)[-1]
check("A4 alpha survives filter", ALPHA_TITLE in tail)
check("A5 beta filtered out", BETA_TITLE not in tail)
send_key("\r")                      # resume Alpha
check("A6 resumed transcript replays",
      wait_for(f"seed turn for {ALPHA_TITLE}", timeout=30))

# ── Flow B: current-session marker ────────────────────────────────────────
open_resume()
check("B1 picker reopens", wait_for("Resume session", count=2))
# Scan REVERSED: the buffer holds every repaint; the LAST matching line is
# from the most recent paint, which is the one carrying the live marker.
screen_now = clean(bytes(buf))
alpha_row = next((ln for ln in reversed(screen_now.splitlines()) if ALPHA_TITLE in ln), "")
check("B2 marker on adopted row", "●" in alpha_row)
send_key("\x1b")                    # esc closes without changing session
pump(0.4)

# ── Flow C: auto-titled row visible and searchable ────────────────────────
# (Do NOT assert "New Session" is absent from the whole screen: the shell's
# own boot session is legitimately titled "New Session". The honest proof is
# that the untitled session's row now shows — and fuzzy-matches — its
# derived prompt title.)
open_resume()
check("C1 picker shows derived title", wait_for("Plan the quarterly offsite"))
for ch in "offsite":
    send_key(ch, 0.3)
# Ink repaints differentially, so a surviving unchanged row may not be
# rewritten below the final query paint — pixel-scanning the tail can't see
# it. The honest end-to-end proof: enter resumes items[0], and the transcript
# notice names the session — proving the filter narrowed to (and picked) the
# auto-titled session.
send_key("\r")
check("C2 fuzzy search resumed the auto-titled session",
      wait_for('Resumed “Plan the quarterly offsite”', timeout=20))
pump(0.4)

# ── Teardown ──────────────────────────────────────────────────────────────
os.write(fd, b"\x03")  # ctrl+c exit shell
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
