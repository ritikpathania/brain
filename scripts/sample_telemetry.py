#!/usr/bin/env python3
import sys
import os
import time
import subprocess
import json

def get_process_metrics(pid):
    try:
        # RSS & CPU
        ps_out = subprocess.check_output(["ps", "-p", str(pid), "-o", "rss,%cpu"], text=True).strip().splitlines()
        if len(ps_out) < 2:
            return None
        rss, cpu = ps_out[1].strip().split()
        
        # Threads
        threads_out = subprocess.check_output(["ps", "-M", str(pid)], text=True).strip().splitlines()
        threads = max(1, len(threads_out) - 1)

        # Open File Descriptors
        fds_out = subprocess.check_output(["lsof", "-p", str(pid)], text=True).strip().splitlines()
        fds = max(0, len(fds_out) - 1)

        return {
            "rss_kb": int(rss),
            "cpu_percent": float(cpu),
            "threads": threads,
            "open_fds": fds
        }
    except Exception:
        return None

def check_socket_health():
    try:
        res = subprocess.run(["./target/release/brain", "health"], capture_output=True, text=True)
        return res.returncode == 0
    except Exception:
        return False

def main():
    if len(sys.argv) < 2:
        print("Usage: sample_telemetry.py <pid> [output_jsonl]")
        sys.exit(1)

    pid = int(sys.argv[1])
    output_file = sys.argv[2] if len(sys.argv) > 2 else "target/soak_samples.jsonl"
    os.makedirs(os.path.dirname(os.path.abspath(output_file)), exist_ok=True)

    metrics = get_process_metrics(pid)
    socket_ok = check_socket_health()

    if metrics is None:
        print(f"Process {pid} not found")
        sys.exit(1)

    sample = {
        "timestamp": time.time(),
        "pid": pid,
        "metrics": metrics,
        "socket_healthy": socket_ok
    }

    with open(output_file, "a") as f:
        f.write(json.dumps(sample) + "\n")

    print(json.dumps(sample))

if __name__ == "__main__":
    main()
