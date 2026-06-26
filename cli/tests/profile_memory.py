# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "pyte",
# ]
# ///

import os
import pty
import subprocess
import time
import socket
import json
import sys
import platform
import threading
import select

TESTS_DIR = os.path.dirname(os.path.abspath(__file__))
HISTORY_DIR = os.path.join(TESTS_DIR, "history")
os.makedirs(HISTORY_DIR, exist_ok=True)
CLI_DIR = os.path.dirname(TESTS_DIR)
REPO_ROOT = os.path.dirname(CLI_DIR)

class MockDaemon:
    def __init__(self, socket_path):
        self.socket_path = socket_path
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        if os.path.exists(socket_path):
            os.remove(socket_path)
        os.makedirs(os.path.dirname(socket_path), exist_ok=True)
        self.sock.bind(socket_path)
        self.sock.listen(5)
        self.running = True
        self.thread = threading.Thread(target=self.accept_loop, daemon=True)
        self.thread.start()

    def accept_loop(self):
        self.sock.settimeout(0.5)
        while self.running:
            try:
                conn, _ = self.sock.accept()
                t = threading.Thread(target=self.handle_client, args=(conn,), daemon=True)
                t.start()
            except socket.timeout:
                continue
            except Exception:
                break

    def handle_client(self, conn):
        buffer = b""
        while self.running:
            try:
                data = conn.recv(1024)
                if not data:
                    break
                buffer += data
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    if line:
                        self.send_response(conn)
            except Exception:
                break
        conn.close()

    def send_response(self, conn):
        try:
            stream_id = "mem_test_stream"
            conn.sendall(f'{{"type":"stream_start","streamId":"{stream_id}"}}\n'.encode())
            time.sleep(0.01)
            conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":1,"content":"Memory check response data chunk"}}\n'.encode())
            time.sleep(0.01)
            conn.sendall(f'{{"type":"stream_end","streamId":"{stream_id}","sequence":2}}\n'.encode())
        except socket.error:
            pass

    def close(self):
        self.running = False
        try:
            self.sock.close()
        except:
            pass
        if os.path.exists(self.socket_path):
            try:
                os.remove(self.socket_path)
            except:
                pass

def get_git_info():
    try:
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT).decode().strip()
        branch = subprocess.check_output(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=REPO_ROOT).decode().strip()
        return commit, branch
    except:
        return "unknown", "unknown"

def get_tool_versions():
    bun_v = "unknown"
    rustc_v = "unknown"
    try:
        bun_v = subprocess.check_output(["bun", "--version"]).decode().strip()
    except:
        pass
    try:
        rustc_v = subprocess.check_output(["rustc", "--version"]).decode().strip().split(" ")[1]
    except:
        pass
    return bun_v, rustc_v

def run_profile(socket_path, force_gc=False):
    master_fd, slave_fd = pty.openpty()
    
    # Clean old run file if exists
    run_file = os.path.join(TESTS_DIR, "memory_profile_run.json")
    if os.path.exists(run_file):
        os.remove(run_file)
        
    env = {
        **os.environ,
        "BRAIN_SOCKET_PATH": socket_path,
        "BRAIN_MEM_PROFILE": "true",
        "BRAIN_MEM_PROFILE_PATH": run_file,
        "BRAIN_FORCE_GC": "true" if force_gc else "false",
        "TERM": "xterm-256color"
    }
    
    proc = subprocess.Popen(
        ["bun", "run", "src/main.tsx"],
        cwd=CLI_DIR,
        stdin=slave_fd,
        stdout=slave_fd,
        stderr=subprocess.PIPE,
        env=env
    )
    os.close(slave_fd)
    
    # Wait for process to exit
    print(f"Waiting for profiling run (GC={force_gc}) to complete 100 queries...")
    start_time = time.time()
    while proc.poll() is None:
        if time.time() - start_time > 60:
            print("WARNING: Profiling run timed out!")
            proc.terminate()
            break
        try:
            r, _, _ = select.select([master_fd], [], [], 0.1)
            if r:
                os.read(master_fd, 4096)
        except:
            time.sleep(0.1)
            
    os.close(master_fd)
    
    records = []
    if os.path.exists(run_file):
        try:
            with open(run_file, "r") as f:
                records = json.load(f)
        except Exception as e:
            print(f"Error loading run results: {e}")
            
    return records

def main():
    socket_path = "/tmp/mock_mem_daemon.sock"
    daemon = MockDaemon(socket_path)
    
    try:
        # Run A: Baseline (no forced GC)
        print("Starting Run A (Baseline, GC=False)...")
        records_a = run_profile(socket_path, force_gc=False)
        
        # Run B: Forced GC
        print("\nStarting Run B (Forced GC=True)...")
        records_b = run_profile(socket_path, force_gc=True)
        
    finally:
        daemon.close()
        
    if not records_a or not records_b:
        print("ERROR: Failed to collect memory profiling records!")
        sys.exit(1)
        
    init_a = next((r for r in records_a if r["queryCount"] == 10), records_a[0])
    final_a = records_a[-1]
    
    init_b = next((r for r in records_b if r["queryCount"] == 10), records_b[0])
    final_b = records_b[-1]
    
    rss_growth_a = final_a["rss"] - init_a["rss"]
    heap_growth_a = final_a["heapUsed"] - init_a["heapUsed"]
    
    rss_growth_b = final_b["rss"] - init_b["rss"]
    heap_growth_b = final_b["heapUsed"] - init_b["heapUsed"]
    
    commit, branch = get_git_info()
    bun_v, rustc_v = get_tool_versions()
    
    report = {
        "schemaVersion": 1,
        "metadata": {
            "gitCommit": commit,
            "gitBranch": branch,
            "timestamp": int(time.time()),
            "bunVersion": bun_v,
            "rustVersion": rustc_v,
            "os": platform.platform(),
            "cpuArchitecture": platform.machine()
        },
        "results": {
            "baseline": {
                "initial_rss_mb": init_a["rss"],
                "final_rss_mb": final_a["rss"],
                "rss_growth_mb": rss_growth_a,
                "initial_heap_used_mb": init_a["heapUsed"],
                "final_heap_used_mb": final_a["heapUsed"],
                "heap_growth_mb": heap_growth_a
            },
            "forced_gc": {
                "initial_rss_mb": init_b["rss"],
                "final_rss_mb": final_b["rss"],
                "rss_growth_mb": rss_growth_b,
                "initial_heap_used_mb": init_b["heapUsed"],
                "final_heap_used_mb": final_b["heapUsed"],
                "heap_growth_mb": heap_growth_b
            }
        },
        "timeSeries": {
            "baseline": records_a,
            "forced_gc": records_b
        }
    }
    
    filename = f"mem_profile_{int(time.time())}.json"
    archive_path = os.path.join(HISTORY_DIR, filename)
    with open(archive_path, "w") as f:
        json.dump(report, f, indent=2)
        
    print(f"\n======================================")
    print(f"MEMORY PROFILING REPORT ARCHIVED")
    print(f"Location: {archive_path}")
    print(f"======================================")
    print(f"Baseline Run (GC=False):")
    print(f"  - RSS Growth (Q10->Q100): {rss_growth_a:+.2f} MB (Initial: {init_a['rss']:.1f} MB, Final: {final_a['rss']:.1f} MB)")
    print(f"  - Heap Used Growth:      {heap_growth_a:+.2f} MB (Initial: {init_a['heapUsed']:.1f} MB, Final: {final_a['heapUsed']:.1f} MB)")
    print(f"Forced GC Run (GC=True):")
    print(f"  - RSS Growth (Q10->Q100): {rss_growth_b:+.2f} MB (Initial: {init_b['rss']:.1f} MB, Final: {final_b['rss']:.1f} MB)")
    print(f"  - Heap Used Growth:      {heap_growth_b:+.2f} MB (Initial: {init_b['heapUsed']:.1f} MB, Final: {final_b['heapUsed']:.1f} MB)")
    
    if heap_growth_b < 5.0 and rss_growth_b < 10.0:
        print("\nCONCLUSION: Memory growth is STABLE under forced GC. No true JS heap or native memory leak.")
    else:
        print("\nWARNING: High memory growth detected even with forced GC! Potential memory leak present.")
        
    sys.exit(0)

if __name__ == "__main__":
    main()
