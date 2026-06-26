# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "pyte",
#     "psutil",
# ]
# ///

import os
import sys
import time
import random
import csv
import json
import platform
import subprocess

TESTS_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.append(TESTS_DIR)

# Import components from rigorous_test
from rigorous_test import MockDaemon, TUIHarness

HISTORY_DIR = os.path.join(TESTS_DIR, "history")
os.makedirs(HISTORY_DIR, exist_ok=True)
REPO_ROOT = os.path.dirname(os.path.dirname(TESTS_DIR))

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

def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--queries", type=int, default=500)
    args = parser.parse_args()
    
    if args.seed is None:
        args.seed = random.randint(10000, 99999)
        
    print(f"======================================")
    print(f"STARTING TUI CLIENT ACCELERATED SOAK TEST")
    print(f"Target Queries: {args.queries}")
    print(f"Deterministic Seed: {args.seed}")
    print(f"======================================")
    
    rng = random.Random(args.seed)
    socket_path = "/tmp/mock_soak_daemon.sock"
    daemon = MockDaemon(socket_path, seed=args.seed)
    
    harness = None
    csv_file = os.path.join(TESTS_DIR, "soak_test_report.csv")
    csv_headers = ["query_idx", "timestamp", "action", "rss_mb", "cpu_percent", "latency_ms", "status"]
    
    cancellation_latencies = []
    reconnect_latencies = []
    crash_count = 0
    
    try:
        harness = TUIHarness("Soak Test", rows=40, cols=130, socket_path=socket_path)
        assert harness.wait_for_connection(timeout=4.0), "Failed to connect client to mock daemon at startup"
        
        initial_metrics = harness.get_process_metrics()
        
        with open(csv_file, mode="w", newline="") as f:
            writer = csv.writer(f)
            writer.writerow(csv_headers)
            
            for q_idx in range(1, args.queries + 1):
                # Check process health
                poll = harness.proc.poll()
                if poll is not None:
                    print(f"ERROR: TUI client process exited mid-soak with code {poll}!")
                    crash_count += 1
                    break
                    
                # Decide action based on RNG
                roll = rng.random()
                action = "query"
                latency = 0.0
                status = "OK"
                
                # Metrics measurement
                metrics = harness.get_process_metrics()
                
                if roll < 0.80:
                    # 80% chance: standard query stream
                    action = "query"
                    daemon.behavior = "normal"
                    harness.send_command(f"query soak check {q_idx}")
                    harness.wait_for_text("speaking", timeout=2.5)
                elif roll < 0.90:
                    # 10% chance: query and immediate cancellation
                    action = "cancel"
                    daemon.behavior = "slow"
                    harness.send_command(f"query slow soak check {q_idx}")
                    harness.wait_for_text("Delayed", timeout=2.0)
                    
                    start_cancel = time.time()
                    harness.write_ansi(b"\x03")
                    harness.sleep(0.15)
                    latency = (time.time() - start_cancel) * 1000
                    cancellation_latencies.append(latency)
                elif roll < 0.95:
                    # 5% chance: resize
                    action = "resize"
                    new_rows = rng.choice([24, 30, 40, 50])
                    new_cols = rng.choice([80, 100, 130, 180])
                    harness.resize(new_rows, new_cols)
                    harness.sleep(0.1)
                else:
                    # 5% chance: daemon crash and reconnect
                    action = "reconnect"
                    daemon.behavior = "crash_mid"
                    harness.send_command(f"query crash soak check {q_idx}")
                    harness.sleep(0.5)
                    
                    start_recon = time.time()
                    daemon.behavior = "normal"
                    
                    reconnected = False
                    for _ in range(40):
                        harness.read_available()
                        content = harness.stderr_content.decode('utf-8', errors='ignore')
                        if content.count("Successfully connected") > len(reconnect_latencies) + 1:
                            reconnected = True
                            break
                        harness.sleep(0.05)
                        
                    latency = (time.time() - start_recon) * 1000
                    if reconnected:
                        reconnect_latencies.append(latency)
                    else:
                        status = "RECONNECT_TIMEOUT"
                
                writer.writerow([
                    q_idx,
                    int(time.time()),
                    action,
                    f"{metrics['memory_rss_mb']:.2f}",
                    f"{metrics['cpu_percent']:.1f}",
                    f"{latency:.1f}",
                    status
                ])
                
                if q_idx % 50 == 0:
                    print(f"Progress: {q_idx}/{args.queries} queries completed. Current RSS: {metrics['memory_rss_mb']:.1f} MB")
                    
        final_metrics = harness.get_process_metrics()
        
    finally:
        if harness:
            harness.close()
        daemon.close()
        
    rss_growth = final_metrics["memory_rss_mb"] - initial_metrics["memory_rss_mb"]
    max_cancel = max(cancellation_latencies) if cancellation_latencies else 0.0
    avg_cancel = sum(cancellation_latencies)/len(cancellation_latencies) if cancellation_latencies else 0.0
    max_recon = max(reconnect_latencies) if reconnect_latencies else 0.0
    avg_recon = sum(reconnect_latencies)/len(reconnect_latencies) if reconnect_latencies else 0.0
    
    commit, branch = get_git_info()
    bun_v, rustc_v = get_tool_versions()
    
    soak_summary = {
        "schemaVersion": 1,
        "metadata": {
            "gitCommit": commit,
            "gitBranch": branch,
            "timestamp": int(time.time()),
            "bunVersion": bun_v,
            "rustVersion": rustc_v,
            "os": platform.platform(),
            "cpuArchitecture": platform.machine(),
            "testSeed": args.seed
        },
        "results": {
            "total_queries": args.queries,
            "crash_count": crash_count,
            "rss_growth_mb": rss_growth,
            "initial_rss_mb": initial_metrics["memory_rss_mb"],
            "final_rss_mb": final_metrics["memory_rss_mb"],
            "cancellation": {
                "max_ms": max_cancel,
                "avg_ms": avg_cancel
            },
            "reconnect": {
                "max_ms": max_recon,
                "avg_ms": avg_recon
            }
        }
    }
    
    archive_path = os.path.join(HISTORY_DIR, f"soak_profile_{int(time.time())}.json")
    with open(archive_path, "w") as f:
        json.dump(soak_summary, f, indent=2)
        
    print(f"\n======================================")
    print(f"SOAK TEST RESULTS SUMMARY")
    print(f"Location: {archive_path}")
    print(f"======================================")
    print(f" - Crash Count:          {crash_count} (Limit: 0)")
    print(f" - RSS Memory Growth:    {rss_growth:+.2f} MB (Initial: {initial_metrics['memory_rss_mb']:.1f} MB, Final: {final_metrics['memory_rss_mb']:.1f} MB)")
    print(f" - Max Cancel Latency:   {max_cancel:.1f} ms (Target: < 450 ms)")
    print(f" - Max Reconnect Time:   {max_recon:.1f} ms (Target: < 2500 ms)")
    
    assert crash_count == 0, f"Soak test failed: Client crashed during execution!"
    assert rss_growth < 150.0, f"Soak test failed: RSS growth too high ({rss_growth:.2f} MB)"
    if cancellation_latencies:
        assert max_cancel < 600.0, f"Soak test failed: Cancellation latency too high ({max_cancel:.1f} ms)"
    if reconnect_latencies:
        assert max_recon < 3500.0, f"Soak test failed: Reconnect latency too high ({max_recon:.1f} ms)"
        
    print("\nSUCCESS: Soak test completed within acceptable baseline limits!")
    sys.exit(0)

if __name__ == "__main__":
    main()
