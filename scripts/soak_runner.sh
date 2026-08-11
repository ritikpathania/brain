#!/usr/bin/env bash
set -e

CYCLES="${1:-15}"
OUTPUT_REPORT="${2:-target/soak_report.json}"
SAMPLES_FILE="target/soak_samples.jsonl"

rm -f "$SAMPLES_FILE"
mkdir -p target

echo "Building release binaries for soak test..."
cargo build --release --bin brain --bin brain-daemon

echo "Starting daemon for steady-state soak test..."
./target/release/brain-daemon daemon run > /tmp/brain_soak_daemon.log 2>&1 &
DAEMON_PID=$!

# Wait for socket health
for i in {1..30}; do
    if ./target/release/brain health > /dev/null 2>&1; then break; fi
    sleep 0.1
done

echo "Daemon started (PID: $DAEMON_PID). Beginning $CYCLES soak cycles..."

for ((i=1; i<=CYCLES; i++)); do
    # 1. Ingest observation
    ./target/release/brain ingest "[soak-cycle-$i] Reliability soak observation index $i at $(date)" > /dev/null
    
    # 2. Query relational memory graph with latency timing
    Q_START=$(python3 -c 'import time; print(time.time())')
    ./target/release/brain query "soak observation" > /dev/null
    Q_END=$(python3 -c 'import time; print(time.time())')
    Q_LATENCY_MS=$(python3 -c "print(round(($Q_END - $Q_START) * 1000, 2))")
    
    # 3. Sample process telemetry
    python3 -c "
import sys, os, time, subprocess, json

pid = $DAEMON_PID
q_latency = $Q_LATENCY_MS
output_file = '$SAMPLES_FILE'

try:
    ps_out = subprocess.check_output(['ps', '-p', str(pid), '-o', 'rss,%cpu'], text=True).strip().splitlines()
    rss, cpu = ps_out[1].strip().split()
    threads = max(1, len(subprocess.check_output(['ps', '-M', str(pid)], text=True).strip().splitlines()) - 1)
    fds = max(0, len(subprocess.check_output(['lsof', '-p', str(pid)], text=True).strip().splitlines()) - 1)
    res = subprocess.run(['./target/release/brain', 'health'], capture_output=True, text=True)
    socket_ok = (res.returncode == 0)

    sample = {
        'timestamp': time.time(),
        'pid': pid,
        'metrics': {
            'rss_kb': int(rss),
            'cpu_percent': float(cpu),
            'threads': threads,
            'open_fds': fds,
            'query_latency_ms': q_latency
        },
        'socket_healthy': socket_ok
    }
    with open(output_file, 'a') as f:
        f.write(json.dumps(sample) + '\n')
except Exception as e:
    pass
"
    
    # 4. Short pause to simulate realistic workload
    sleep 0.2
done

echo "Soak loop completed. Initiating graceful shutdown via SIGTERM..."
kill -TERM "$DAEMON_PID" || true

# Wait up to 3 seconds for graceful shutdown
for i in {1..30}; do
    if ! kill -0 "$DAEMON_PID" 2>/dev/null; then break; fi
    sleep 0.1
done

# Fallback to SIGKILL if still running
if kill -0 "$DAEMON_PID" 2>/dev/null; then
    echo "⚠️ Daemon did not exit gracefully; sending SIGKILL"
    kill -9 "$DAEMON_PID" || true
else
    echo "✔ Daemon shut down gracefully"
fi

echo "Generating steady-state soak report JSON..."
python3 -c '
import json, sys

samples = []
with open("target/soak_samples.jsonl") as f:
    for line in f:
        if line.strip():
            samples.append(json.loads(line))

if not samples:
    sys.exit(1)

rss_list = [s["metrics"]["rss_kb"] for s in samples]
cpu_list = [s["metrics"]["cpu_percent"] for s in samples]
fd_list = [s["metrics"]["open_fds"] for s in samples]
thread_list = [s["metrics"]["threads"] for s in samples]
lat_list = [s["metrics"].get("query_latency_ms", 0.0) for s in samples]

steady_rss_start = rss_list[len(rss_list)//2]
steady_rss_end = rss_list[-1]
rss_growth_pct = ((steady_rss_end - steady_rss_start) / steady_rss_start) * 100 if steady_rss_start > 0 else 0

lat_start = lat_list[0] if lat_list else 0
lat_end = lat_list[-1] if lat_list else 0
latency_drift_ms = round(lat_end - lat_start, 2)

report = {
    "tier": "CI_Baseline",
    "steady_state": {
        "cycles": len(samples),
        "rss_start_kb": rss_list[0],
        "rss_peak_kb": max(rss_list),
        "rss_final_kb": rss_list[-1],
        "rss_steady_growth_pct": round(rss_growth_pct, 2),
        "max_open_fds": max(fd_list),
        "fd_delta": fd_list[-1] - fd_list[0],
        "max_threads": max(thread_list),
        "thread_delta": thread_list[-1] - thread_list[0],
        "initial_query_latency_ms": lat_start,
        "final_query_latency_ms": lat_end,
        "query_latency_drift_ms": latency_drift_ms,
        "socket_health_rate": sum(1 for s in samples if s["socket_healthy"]) / len(samples)
    }
}

with open("'"$OUTPUT_REPORT"'", "w") as out:
    json.dump(report, out, indent=2)

print(json.dumps(report, indent=2))
'

echo "Saved soak report to $OUTPUT_REPORT"
