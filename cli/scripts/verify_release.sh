#!/bin/bash
# verify_release.sh - CI release packaging and validation script
set -euo pipefail

# Ensure we are in the repository root directory
REPO_ROOT="/Users/ritikpathania/Developer/PyCharm/brain"
cd "$REPO_ROOT"

echo "=================================================="
# 1. Run package.sh to create the release tarball
echo "Step 1: Running package.sh..."
./package.sh

# 2. Dynamically resolve the generated tarball
TAR_FILE=$(ls brain-*.tar.gz | head -n 1)
if [ -z "$TAR_FILE" ]; then
    echo "ERROR: Generated release tarball not found!"
    exit 1
fi
echo "Step 2: Resolving generated tarball: $TAR_FILE"

# 3. Create a temporary isolated workspace
TMP_WORKSPACE=$(mktemp -d)
echo "Step 3: Creating temporary workspace: $TMP_WORKSPACE"

# Set custom HOME to keep config/PID/socket files local to the workspace
export HOME="$TMP_WORKSPACE"
export BRAIN_SOCKET_PATH="$TMP_WORKSPACE/.brain/daemon.sock"

# 4. Extract tarball to the workspace
echo "Step 4: Extracting release package..."
tar -xzf "$TAR_FILE" -C "$TMP_WORKSPACE"

# Verify files exist in tarball
for file in brain INSTALL.md UPGRADE.md README.md; do
    if [ ! -f "$TMP_WORKSPACE/$file" ]; then
        echo "ERROR: Missing expected file in tarball: $file"
        rm -rf "$TMP_WORKSPACE"
        exit 1
    fi
done

# 5. Spin up the release daemon
echo "Step 5: Starting release daemon..."
"$TMP_WORKSPACE/brain" daemon start

# Wait for the HTTP health server to bind
echo "Waiting for health server to start on http://127.0.0.1:8080..."
MAX_ATTEMPTS=30
SUCCESS=0
for i in $(seq 1 $MAX_ATTEMPTS); do
    if curl -s -f "http://127.0.0.1:8080/health" > /dev/null; then
        echo "Health endpoint is UP!"
        SUCCESS=1
        break
    fi
    sleep 0.2
done

if [ "$SUCCESS" -ne 1 ]; then
    echo "ERROR: Health server did not start on time!"
    echo "Daemon logs:"
    cat "$TMP_WORKSPACE/.brain/daemon.log" || true
    "$TMP_WORKSPACE/brain" daemon stop || true
    rm -rf "$TMP_WORKSPACE"
    exit 1
fi

# 6. Verify that /metrics/json responds correctly with 200 OK
echo "Step 6: Querying /metrics/json..."
METRICS_RESPONSE=$(curl -s -f "http://127.0.0.1:8080/metrics/json")
echo "Metrics Response: $METRICS_RESPONSE"

# Assert fields are present in the JSON response
if ! echo "$METRICS_RESPONSE" | grep -q "cache_hit_rate"; then
    echo "ERROR: Response does not contain expected metrics fields!"
    "$TMP_WORKSPACE/brain" daemon stop || true
    rm -rf "$TMP_WORKSPACE"
    exit 1
fi

echo "SUCCESS: Metrics endpoint validated successfully!"

# 7. Stop the daemon
echo "Step 7: Stopping daemon..."
"$TMP_WORKSPACE/brain" daemon stop

# 8. Clean up
echo "Step 8: Cleaning up..."
rm -rf "$TMP_WORKSPACE"
echo "=================================================="
echo "CI RELEASE VERIFICATION PASSED SUCCESSFULLY!"
exit 0
