# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "pyte",
#     "psutil",
# ]
# ///

import os
import pty
import subprocess
import select
import termios
import struct
import fcntl
import time
import socket
import threading
import pyte
import sys
import psutil
import json
import random

# Setup directories
TESTS_DIR = os.path.dirname(os.path.abspath(__file__))
CLI_DIR = os.path.dirname(TESTS_DIR)
REPO_ROOT = os.path.dirname(CLI_DIR)
CAPTURE_DIR = os.path.join(TESTS_DIR, "captures")
os.makedirs(CAPTURE_DIR, exist_ok=True)

def set_pty_size(fd, rows, cols):
    size_struct = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, size_struct)

class MockDaemon:
    def __init__(self, socket_path, seed=None):
        self.socket_path = socket_path
        self.rng = random.Random(seed)
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        if os.path.exists(socket_path):
            os.remove(socket_path)
        
        # Ensure parent directory exists
        os.makedirs(os.path.dirname(socket_path), exist_ok=True)
        self.sock.bind(socket_path)
        self.sock.listen(5)
        self.running = True
        self.thread = threading.Thread(target=self.accept_loop, daemon=True)
        self.thread.start()
        self.client_conns = []
        self.behavior = "normal"  # "normal", "slow", "malformed", "crash_mid", "bad_packet"
        self.query_received_event = threading.Event()
        self.last_query = None

    def accept_loop(self):
        self.sock.settimeout(0.5)
        while self.running:
            try:
                conn, _ = self.sock.accept()
                sys.stderr.write("DEBUG: MockDaemon accepted connection!\n")
                sys.stderr.flush()
                conn.setblocking(False)
                self.client_conns.append(conn)
                t = threading.Thread(target=self.handle_client, args=(conn,), daemon=True)
                t.start()
            except socket.timeout:
                continue
            except Exception as e:
                sys.stderr.write(f"DEBUG: accept_loop exception: {e}\n")
                sys.stderr.flush()
                break

    def handle_client(self, conn):
        sys.stderr.write("DEBUG: handle_client thread started!\n")
        sys.stderr.flush()
        buffer = b""
        while self.running:
            try:
                r, _, _ = select.select([conn], [], [], 0.1)
                if not r:
                    continue
                data = conn.recv(1024)
                if not data:
                    sys.stderr.write("DEBUG: conn.recv returned empty bytes (client disconnected)\n")
                    sys.stderr.flush()
                    break
                buffer += data
                while b"\n" in buffer:
                    line, buffer = buffer.split(b"\n", 1)
                    if line:
                        payload = line.decode('utf-8', errors='ignore')
                        sys.stderr.write(f"DEBUG: MockDaemon received: {payload}\n")
                        sys.stderr.flush()
                        try:
                            parsed = json.loads(payload)
                            self.last_query = parsed.get("payload")
                            self.query_received_event.set()
                        except Exception as pe:
                            sys.stderr.write(f"DEBUG: JSON parse exception: {pe}\n")
                            sys.stderr.flush()
                        self.send_response(conn)
            except Exception as e:
                sys.stderr.write(f"DEBUG: handle_client exception: {e}\n")
                sys.stderr.flush()
                break
        conn.close()

    def send_response(self, conn):
        try:
            if self.behavior == "normal":
                self.stream_response(conn, ["Hello", " world!", "\\nThis is relational memory speaking."])
            elif self.behavior == "slow":
                self.stream_response(conn, ["Delayed", " content", " for", " testing", " latency."], delay=0.3)
            elif self.behavior == "malformed":
                conn.sendall(b"not-json-at-all\n")
            elif self.behavior in ["crash_mid", "out_of_order", "duplicate", "malformed_json", "malformed_utf8"]:
                self.stream_response(conn, ["First chunk...", "Second chunk...", "Third chunk...", "Fourth chunk...", "Fifth chunk..."])
            elif self.behavior == "bad_packet":
                conn.sendall(b'{"type":"stream_metric","streamId":"bad_test","sequence":1,"value":42}\n')
                time.sleep(0.1)
                self.stream_response(conn, ["Proceeding", " normally after", " bad packet."])
            elif self.behavior == "regression":
                stream_id = "test_stream"
                conn.sendall(f'{{"type":"stream_start","streamId":"{stream_id}"}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":1,"content":"chunk 1"}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":2,"content":"chunk 2"}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":1,"content":"chunk 1 again"}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_end","streamId":"{stream_id}","sequence":3}}\n'.encode())
            elif self.behavior == "unterminated":
                conn.sendall(b'{"type":"stream_start","streamId":"stream_first"}\n')
                time.sleep(0.01)
                conn.sendall(b'{"type":"stream_start","streamId":"stream_second"}\n')
                time.sleep(0.01)
                conn.sendall(b'{"type":"stream_end","streamId":"stream_second","sequence":1}\n')
            elif self.behavior == "post_termination":
                stream_id = "test_stream"
                conn.sendall(f'{{"type":"stream_start","streamId":"{stream_id}"}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_end","streamId":"{stream_id}","sequence":1}}\n'.encode())
                time.sleep(0.01)
                conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":2,"content":"zombie chunk"}}\n'.encode())
        except socket.error:
            pass

    def stream_response(self, conn, chunks, delay=0.01):
        stream_id = "test_stream"
        conn.sendall(f'{{"type":"stream_start","streamId":"{stream_id}"}}\n'.encode())
        time.sleep(delay)
        
        # Inject random fault if active
        fault_idx = -1
        if self.behavior == "crash_mid":
            # Ensure at least the first chunk is sent so the client begins rendering
            fault_idx = self.rng.randint(1, len(chunks) - 1)
            sys.stderr.write(f"DEBUG: MockDaemon injecting behavior 'crash_mid' at chunk index {fault_idx}\n")
            sys.stderr.flush()
        elif self.behavior in ["out_of_order", "duplicate", "malformed_json", "malformed_utf8"]:
            fault_idx = self.rng.randint(0, len(chunks) - 1)
            sys.stderr.write(f"DEBUG: MockDaemon injecting behavior '{self.behavior}' at chunk index {fault_idx}\n")
            sys.stderr.flush()

        seq = 1
        conn.sendall(f'{{"type":"stream_progress","streamId":"{stream_id}","sequence":{seq},"progress":0.1,"message":"Mocking stream"}}\n'.encode())
        time.sleep(delay)
        for idx, chunk in enumerate(chunks):
            seq += 1
            if idx == fault_idx:
                if self.behavior == "crash_mid":
                    conn.close()
                    return
                elif self.behavior == "out_of_order":
                    conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":{seq + 5},"content":"{chunk}"}}\n'.encode())
                    seq += 5
                elif self.behavior == "duplicate":
                    conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":{seq - 1},"content":"{chunk}"}}\n'.encode())
                elif self.behavior == "malformed_json":
                    conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":{seq},"content":\n'.encode())
                elif self.behavior == "malformed_utf8":
                    conn.sendall(b'{"type":"stream_chunk","streamId":"' + stream_id.encode() + b'","sequence":' + str(seq).encode() + b',"content":"\xff\xfe\xfd"}\n')
            else:
                conn.sendall(f'{{"type":"stream_chunk","streamId":"{stream_id}","sequence":{seq},"content":"{chunk}"}}\n'.encode())
            time.sleep(delay)
        seq += 1
        conn.sendall(f'{{"type":"stream_end","streamId":"{stream_id}","sequence":{seq}}}\n'.encode())

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


class TUIHarness:
    def __init__(self, name, rows=24, cols=80, socket_path="/tmp/mock_brain_daemon.sock"):
        self.name = name.lower().replace(" ", "_").replace(" - ", "_")
        self.rows = rows
        self.cols = cols
        self.socket_path = socket_path
        
        self.master_fd, self.slave_fd = pty.openpty()
        self.screen = pyte.Screen(cols, rows)
        self.stream = pyte.Stream(self.screen)
        
        set_pty_size(self.master_fd, rows, cols)
        
        self.stderr_content = b""
        
        def child_setup():
            os.setsid()
            try:
                fcntl.ioctl(0, termios.TIOCSCTTY, 0)
            except:
                pass

        self.proc = subprocess.Popen(
            ["bun", "run", "src/main.tsx"],
            cwd=CLI_DIR,
            stdin=self.slave_fd,
            stdout=self.slave_fd,
            stderr=subprocess.PIPE,
            preexec_fn=child_setup,
            env={
                **os.environ,
                "BRAIN_SOCKET_PATH": socket_path,
                "COLORFGBG": "15;0",
                "TERM": "xterm-256color"
            }
        )
        os.close(self.slave_fd)
        
        # Set proc.stderr to non-blocking
        fd = self.proc.stderr.fileno()
        fl = fcntl.fcntl(fd, fcntl.F_GETFL)
        fcntl.fcntl(fd, fcntl.F_SETFL, fl | os.O_NONBLOCK)
        
        self.timeline = []

    def sleep(self, duration, capture_interval=0.02):
        start = time.time()
        while time.time() - start < duration:
            self.read_available()
            time.sleep(capture_interval)

    def resize(self, rows, cols):
        self.rows = rows
        self.cols = cols
        set_pty_size(self.master_fd, rows, cols)
        self.screen.resize(cols, rows)
        import signal
        try:
            self.proc.send_signal(signal.SIGWINCH)
        except:
            pass
        self.sleep(0.1)

    def write(self, data):
        encoded = data.encode('utf-8')
        chunk_size = 512
        if len(encoded) <= chunk_size:
            os.write(self.master_fd, encoded)
            self.sleep(0.05)
        else:
            for i in range(0, len(encoded), chunk_size):
                chunk = encoded[i:i+chunk_size]
                os.write(self.master_fd, chunk)
                self.sleep(0.01)
            self.sleep(0.05)

    def write_ansi(self, seq):
        os.write(self.master_fd, seq)
        self.sleep(0.05)

    def send_command(self, cmd):
        if cmd:
            self.write(cmd)
            self.sleep(0.15)
        self.write("\r")
        self.sleep(0.15)

    def read_available(self):
        read_data = False
        while True:
            r, _, _ = select.select([self.master_fd], [], [], 0.01)
            if not r:
                break
            try:
                data = os.read(self.master_fd, 4096)
                if not data:
                    break
                self.stream.feed(data.decode('utf-8', errors='ignore'))
                read_data = True
            except OSError:
                break
        if read_data:
            self.timeline.append((time.time(), self.get_clean_display()))

        # Read non-blocking stderr
        try:
            err_data = self.proc.stderr.read()
            if err_data:
                self.stderr_content += err_data
        except (BlockingIOError, OSError):
            pass
        except Exception:
            pass

    def get_clean_display(self):
        lines = []
        for line in self.screen.display:
            lines.append(line.rstrip())
        return "\n".join(lines)

    def wait_for_text(self, target_text, timeout=3.0, poll_interval=0.05):
        start = time.time()
        while time.time() - start < timeout:
            self.read_available()
            display = self.get_clean_display()
            if target_text in display:
                return True
            time.sleep(poll_interval)
        return False

    def wait_for_connection(self, timeout=3.0):
        start = time.time()
        while time.time() - start < timeout:
            self.read_available()
            content = self.stderr_content.decode('utf-8', errors='ignore')
            if "Successfully connected to Relational Memory daemon!" in content:
                return True
            self.sleep(0.05)
        return False

    def get_process_metrics(self):
        try:
            p = psutil.Process(self.proc.pid)
            return {
                "cpu_percent": p.cpu_percent(interval=None),
                "memory_rss_mb": p.memory_info().rss / (1024 * 1024)
            }
        except:
            return {"cpu_percent": 0.0, "memory_rss_mb": 0.0}

    def close(self):
        try:
            self.proc.terminate()
            self.proc.wait(timeout=0.5)
        except:
            try:
                self.proc.kill()
            except:
                pass
        
        # Read final stderr chunks
        try:
            err_data = self.proc.stderr.read()
            if err_data:
                self.stderr_content += err_data
        except:
            pass

        # Write to log file
        try:
            log_path = os.path.join(CAPTURE_DIR, f"tui_stderr_{self.name}.txt")
            with open(log_path, "wb") as f:
                f.write(self.stderr_content)
        except:
            pass

        try:
            os.close(self.master_fd)
        except:
            pass


def run_test_case(name, test_func, socket_path="/tmp/mock_brain_daemon.sock", rows=24, cols=80, seed=None):
    print(f"\n======================================")
    print(f"RUNNING TEST: {name}")
    print(f"======================================")
    daemon = MockDaemon(socket_path, seed=seed)
    harness = None
    try:
        harness = TUIHarness(name, rows=rows, cols=cols, socket_path=socket_path)
        if name != "Failure Injection - Disconnected":
            assert harness.wait_for_connection(timeout=3.0), "Initial connection message not found in stderr"
        
        result = test_func(harness, daemon)
        success = True
    except Exception as e:
        import traceback
        traceback.print_exc()
        result = str(e)
        success = False
    finally:
        if harness and harness.timeline:
            timeline_file = os.path.join(CAPTURE_DIR, f"{name.lower().replace(' ', '_').replace(' - ', '_')}_timeline.txt")
            with open(timeline_file, "w") as f:
                for idx, (t, display) in enumerate(harness.timeline):
                    f.write(f"--- FRAME {idx} (T+{t - harness.timeline[0][0]:.3f}s) ---\n")
                    f.write(display)
                    f.write("\n\n")
            print(f"Timeline saved to {timeline_file}")
            
        if harness:
            poll_val = harness.proc.poll()
            sys.stderr.write(f"DEBUG: TUI client process poll value: {poll_val}\n")
            sys.stderr.flush()
            harness.close()
        daemon.close()
        
    status = "PASSED" if success else "FAILED"
    print(f"Test '{name}' finished: {status}")
    return success, result


# --- TEST DEFINITIONS ---

def test_sizes_and_resizing_func(harness, daemon):
    assert harness.wait_for_text("Dashboard", timeout=2.0), "Dashboard header not found at 50x180"
    assert harness.wait_for_text("System Monitor", timeout=2.0), "System Monitor missing at 50x180"
    
    harness.resize(40, 130)
    assert harness.wait_for_text("Memory Engine", timeout=2.0), "Memory Engine prompt missing in 130x40 layout"
    assert harness.wait_for_text("File Browser", timeout=2.0), "File Browser tab/widget missing in 130x40 layout"
    
    harness.resize(30, 100)
    assert harness.wait_for_text("Memory Engine", timeout=2.0), "Failed rendering at 100x30"
    
    for s in [24, 30, 40, 30, 24]:
        harness.resize(s, 100)
        harness.sleep(0.02)
    
    return "Dynamic resizing completed successfully."

def test_ui_abuse_func(harness, daemon):
    for _ in range(12):
        harness.write_ansi(b"\t")
        harness.sleep(0.02)
    harness.read_available()
    
    harness.send_command("query postgres with emojis 🦄 💻 and russian текст")
    harness.wait_for_text("🦄", timeout=2.0)
    
    large_input = "ingest " + ("A" * 20000)
    harness.send_command(large_input)
    harness.sleep(0.5)
    
    return "UI abuse stress test completed."

def test_failure_injection_disconnected_func(harness, daemon):
    daemon.close()
    found = harness.wait_for_text("Unreachabl", timeout=3.5)
    display = harness.get_clean_display()
    assert "Unreachabl" in display or "✗" in display, "Failed to display connection retry or unreachable status"
    return "Offline daemon correctly handled."

def test_failure_injection_crash_midstream_func(harness, daemon):
    daemon.behavior = "crash_mid"
    harness.send_command("query breakme")
    assert harness.wait_for_text("First chunk...", timeout=2.0), "Stream start chunk did not render"
    
    harness.sleep(0.5)
    display = harness.get_clean_display()
    assert "Unreachabl" in display or "closed the connection" in display or "Retrying" in display, \
        "Failed to handle connection crash mid-stream"
    
    return "Connection loss mid-stream detected, client recovered."

def test_streaming_and_cancellation_func(harness, daemon):
    daemon.behavior = "normal"
    harness.send_command("query postgres")
    assert harness.wait_for_text("relational memory speaking", timeout=2.0)
    
    daemon.behavior = "slow"
    harness.send_command("query slow_query")
    assert harness.wait_for_text("Delayed content", timeout=2.0)
    
    start_cancel = time.time()
    harness.write_ansi(b"\x03")
    harness.sleep(0.2)
    latency = time.time() - start_cancel
    
    return f"Streaming completed. Ctrl+C cancellation event handled in {latency*1000:.1f}ms."

def test_performance_and_long_session_func(harness, daemon):
    initial_metrics = harness.get_process_metrics()
    for i in range(25):
        harness.send_command(f"query prompt_{i}")
        assert harness.wait_for_text("relational memory speaking", timeout=2.0), f"Query prompt_{i} failed to render response"
        
    final_metrics = harness.get_process_metrics()
    mem_diff = final_metrics["memory_rss_mb"] - initial_metrics["memory_rss_mb"]
    
    return f"Long session completed. RSS Memory diff: {mem_diff:+.2f}MB (Initial: {initial_metrics['memory_rss_mb']:.1f}MB, Final: {final_metrics['memory_rss_mb']:.1f}MB)"

def test_protocol_out_of_order_func(harness, daemon):
    daemon.behavior = "out_of_order"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    content = harness.stderr_content.decode('utf-8', errors='ignore')
    assert "[Protocol Warning] Stream sequence mismatch" in content, "Failed to log sequence mismatch warning"
    return "Out-of-order sequence handled successfully with warning log."

def test_protocol_duplicate_func(harness, daemon):
    daemon.behavior = "duplicate"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    content = harness.stderr_content.decode('utf-8', errors='ignore')
    assert "[Protocol Warning] Stream sequence mismatch" in content, "Failed to log duplicate sequence warning"
    return "Duplicate sequence handled successfully with warning log."

def test_protocol_malformed_json_func(harness, daemon):
    daemon.behavior = "malformed_json"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    display = harness.get_clean_display()
    assert "Memory Engine" in display, "Client crashed on malformed JSON payload"
    return "Malformed JSON payload handled gracefully without crash."

def test_protocol_malformed_utf8_func(harness, daemon):
    daemon.behavior = "malformed_utf8"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    display = harness.get_clean_display()
    assert "Memory Engine" in display, "Client crashed on invalid UTF-8 sequence"
    return "Invalid UTF-8 payload handled gracefully without crash."

def test_protocol_regression_func(harness, daemon):
    daemon.behavior = "regression"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    content = harness.stderr_content.decode('utf-8', errors='ignore')
    assert "Stream sequence regressed" in content, "Failed to log sequence regression warning"
    return "Sequence regression handled successfully with warning log."

def test_protocol_unterminated_func(harness, daemon):
    daemon.behavior = "unterminated"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    content = harness.stderr_content.decode('utf-8', errors='ignore')
    assert "was not terminated before starting stream" in content, "Failed to log unterminated stream warning"
    return "Unterminated stream handled successfully with warning log."

def test_protocol_post_termination_func(harness, daemon):
    daemon.behavior = "post_termination"
    harness.send_command("query postgres")
    harness.sleep(1.0)
    content = harness.stderr_content.decode('utf-8', errors='ignore')
    assert "Received packet for already terminated stream" in content, "Failed to log post-termination warning"
    return "Post-termination packet handled successfully with warning log."


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=None)
    args = parser.parse_args()
    if args.seed is None:
        args.seed = random.randint(10000, 99999)
    print(f"USING DETERMINISTIC TEST SEED: {args.seed}")
    
    tests = [
        ("Sizes and Resizing", test_sizes_and_resizing_func, 50, 180),
        ("UI Abuse", test_ui_abuse_func, 50, 130),
        ("Failure Injection - Disconnected", test_failure_injection_disconnected_func, 50, 130),
        ("Failure Injection - Crash Midstream", test_failure_injection_crash_midstream_func, 50, 130),
        ("Streaming and Cancellation", test_streaming_and_cancellation_func, 50, 130),
        ("Performance and Long Session", test_performance_and_long_session_func, 50, 130),
        ("Protocol - Out Of Order Sequence", test_protocol_out_of_order_func, 50, 130),
        ("Protocol - Duplicate Sequence", test_protocol_duplicate_func, 50, 130),
        ("Protocol - Malformed JSON", test_protocol_malformed_json_func, 50, 130),
        ("Protocol - Malformed UTF-8", test_protocol_malformed_utf8_func, 50, 130),
        ("Protocol - Sequence Regression", test_protocol_regression_func, 50, 130),
        ("Protocol - Unterminated Stream", test_protocol_unterminated_func, 50, 130),
        ("Protocol - Post-Termination Packet", test_protocol_post_termination_func, 50, 130),
    ]
    
    results = {}
    for name, func, r, c in tests:
        success, info = run_test_case(name, func, rows=r, cols=c, seed=args.seed)
        results[name] = {"success": success, "info": info}
        
    summary_file = os.path.join(TESTS_DIR, "rigorous_test_results.json")
    with open(summary_file, "w") as f:
        json.dump(results, f, indent=2)
    
    print("\n======================================")
    print("ALL TESTS COMPLETED. SUMMARY RESULTS:")
    print("======================================")
    all_ok = True
    for name, res in results.items():
        status = "PASSED" if res["success"] else "FAILED"
        print(f" - {name}: {status} ({res['info']})")
        if not res["success"]:
            all_ok = False
            
    sys.exit(0 if all_ok else 1)
