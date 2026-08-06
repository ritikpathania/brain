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
    echo "--- Cycle $i/$CYCLES ---"
    # 1. Ingest observation
    ./target/release/brain ingest "[soak-cycle-$i] Reliability soak observation index $i at $(date)" > /dev/null
    
    # 2. Query relational memory graph
    ./target/release/brain query "soak observation" > /dev/null
    
    # 3. Sample process telemetry
    ./scripts/sample_telemetry.py $DAEMON_PID "$SAMPLES_FILE"
    
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

steady_rss_start = rss_list[len(rss_list)//2]
steady_rss_end = rss_list[-1]
rss_growth_pct = ((steady_rss_end - steady_rss_start) / steady_rss_start) * 100 if steady_rss_start > 0 else 0

report = {
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
        "socket_health_rate": sum(1 for s in samples if s["socket_healthy"]) / len(samples)
    }
}

with open("'"$OUTPUT_REPORT"'", "w") as out:
    json.dump(report, out, indent=2)

print(json.dumps(report, indent=2))
'

echo "Saved soak report to $OUTPUT_REPORT"
