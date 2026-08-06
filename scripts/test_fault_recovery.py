#!/usr/bin/env python3
import subprocess
import time
import os
import sys
import json

def run_cmd(cmd):
    res = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return res.returncode, res.stdout, res.stderr

def main():
    print("=== Fault Recovery & Failure Injection Test Suite ===")
    results = {}
    failed = False

    # 1. Build release binaries
    print("[Scenario 1] Building release binaries...")
    code, _, _ = run_cmd("cargo build --release --bin brain --bin brain-daemon")
    if code != 0:
        print("❌ Build failed")
        sys.exit(1)

    # 2. Daemon restart resilience
    print("[Scenario 2] Daemon restart & UDS socket recovery...")
    run_cmd("./target/release/brain daemon stop")
    code, _, _ = run_cmd("./target/release/brain daemon start")
    time.sleep(1)
    
    code, _, _ = run_cmd("./target/release/brain health")
    if code != 0:
        print("❌ Health check failed after daemon start")
        failed = True

    # Ingest data
    code, _, _ = run_cmd('./target/release/brain ingest "Fault recovery test memory"')
    if code != 0:
        print("❌ Ingest failed before restart")
        failed = True

    # Restart daemon
    run_cmd("./target/release/brain daemon stop")
    time.sleep(1)
    code, _, _ = run_cmd("./target/release/brain daemon start")
    time.sleep(1)

    # Query after restart
    code, out, _ = run_cmd('./target/release/brain query "Fault recovery"')
    if code != 0 or "Fault recovery test memory" not in out:
        print(f"❌ Memory graph retrieval failed after daemon restart: {out}")
        results["daemon_restart_recovery"] = False
        failed = True
    else:
        print("✔ Daemon restart memory persistence verified")
        results["daemon_restart_recovery"] = True

    # 3. Socket file deletion recovery
    print("[Scenario 3] Abrupt socket file deletion...")
    sock_path = os.path.expanduser("~/.brain/daemon.sock")
    if os.path.exists(sock_path):
        os.remove(sock_path)
    
    # Restart daemon to recreate socket
    run_cmd("./target/release/brain daemon stop")
    time.sleep(0.5)
    run_cmd("./target/release/brain daemon start")
    time.sleep(1)

    code, _, _ = run_cmd("./target/release/brain health")
    if code != 0:
        print("❌ Health check failed after socket deletion recovery")
        results["socket_deletion_recovery"] = False
        failed = True
    else:
        print("✔ Socket file deletion recovery verified")
        results["socket_deletion_recovery"] = True

    # Cleanup daemon
    run_cmd("./target/release/brain daemon stop")

    output_report = {
        "fault_recovery": results,
        "all_passed": not failed
    }

    os.makedirs("target", exist_ok=True)
    with open("target/fault_recovery_report.json", "w") as f:
        json.dump(output_report, f, indent=2)

    if failed:
        print("\n❌ Fault recovery test suite failed!")
        sys.exit(1)
    else:
        print("\n✅ Fault recovery test suite passed (100% recovery)!")
        sys.exit(0)

if __name__ == "__main__":
    main()
