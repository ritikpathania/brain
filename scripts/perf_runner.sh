#!/usr/bin/env bash
set -e

OUTPUT_JSON="${1:-target/perf_baseline.json}"
mkdir -p target

echo "Building release binaries..."
cargo build --release --bin brain --bin brain-daemon

echo "Measuring Daemon startup latency via socket health polling..."
START_TIME=$(python3 -c 'import time; print(int(time.time()*1000))')

./target/release/brain-daemon daemon run > /tmp/brain_daemon_perf.log 2>&1 &
DAEMON_PID=$!

# Poll brain health until socket responds OK (max 5 seconds timeout)
HEALTHY=false
for i in {1..50}; do
    if ./target/release/brain health > /dev/null 2>&1; then
        HEALTHY=true
        break
    fi
    sleep 0.1
done

END_TIME=$(python3 -c 'import time; print(int(time.time()*1000))')

if [ "$HEALTHY" = false ]; then
    echo "❌ Daemon failed to become healthy within 5 seconds"
    kill -9 $DAEMON_PID || true
    exit 1
fi

STARTUP_MS=$((END_TIME - START_TIME))

echo "Daemon healthy in ${STARTUP_MS}ms. Measuring idle CPU and steady-state sampled RSS..."
sleep 2

IDLE_CPU=$(ps -p $DAEMON_PID -o %cpu | tail -n 1 | tr -d ' ')
SAMPLED_RSS_KB=$(ps -p $DAEMON_PID -o rss | tail -n 1 | tr -d ' ')
OS_NAME=$(uname -s)
ARCH_NAME=$(uname -m)
RUST_VER=$(rustc --version | head -n 1)

GIT_COMMIT=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
GIT_DIRTY=$(git status --porcelain 2>/dev/null | grep -q . && echo "true" || echo "false")

kill -9 $DAEMON_PID || true

cat <<EOF > "$OUTPUT_JSON"
{
  "schema_version": "1.1",
  "timestamp": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
  "git": {
    "commit": "$GIT_COMMIT",
    "branch": "$GIT_BRANCH",
    "dirty": $GIT_DIRTY
  },
  "environment": {
    "os": "$OS_NAME",
    "architecture": "$ARCH_NAME",
    "rust_version": "$RUST_VER",
    "profile": "release"
  },
  "metrics": {
    "cold_startup_ms": $STARTUP_MS,
    "sampled_rss_kb": $SAMPLED_RSS_KB,
    "idle_cpu_percent": $IDLE_CPU
  }
}
EOF

echo "Saved versioned perf report (v1.1 with git tracking) to $OUTPUT_JSON"
cat "$OUTPUT_JSON"
