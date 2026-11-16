#!/usr/bin/env bash
# Run all required real-daemon and native launcher frontend E2E tests.
set -euo pipefail

for variable in BRAIN_CLI_BIN BRAIN_DAEMON_BIN; do
    binary="${!variable:-}"
    if [[ "$binary" != /* || ! -f "$binary" || ! -x "$binary" ]]; then
        echo "error: $variable must name an absolute executable file" >&2
        exit 2
    fi
done

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$repository_root/packages/brain-frontend"

# Bun writes test results to stderr. Keep the runner's failure status through tee.
output_file="$(mktemp)"
trap 'rm -f "$output_file"' EXIT
NO_COLOR=1 FORCE_COLOR=0 bun run test:e2e 2>&1 | tee "$output_file"
if grep -Ei '(\((skip|todo)\)|^[[:space:]]*[1-9][0-9]* (skip|todo))' "$output_file"; then
    echo "error: required first-party real-daemon E2E was skipped" >&2
    exit 1
fi
if ! grep -Eq '^[[:space:]]*[1-9][0-9]* pass[[:space:]]*$' "$output_file"; then
    echo "error: first-party real-daemon E2E reported no passing tests" >&2
    exit 1
fi
