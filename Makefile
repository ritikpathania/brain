.PHONY: setup build-bridge build-daemon run-daemon lint format type-check

# Setup all dependencies and environments
setup:
	@echo "Syncing daemon virtual environment..."
	cd daemon && uv sync

# Compile the PyO3 Rust extension module for Python
build-bridge:
	cd daemon && uv run maturin develop

# Compile the Rust daemon binary
build-daemon:
	PYO3_PYTHON=$(shell pwd)/daemon/.venv/bin/python cargo build --package brain-v2

# Start the background Rust IPC socket daemon
run-daemon: build-daemon
	PYO3_PYTHON=$(shell pwd)/daemon/.venv/bin/python cargo run --package brain-v2 daemon

# Lint python code using Ruff
lint:
	cd daemon && uv run ruff check .

# Format python code using Ruff
format:
	cd daemon && uv run ruff format .

# Type-check python code using ty and pyrefly
type-check:
	cd daemon && uv run ty check && uv run pyrefly check

# Build standalone Rust daemon binary and copy to root
build-brain:
	PYO3_PYTHON=$(shell pwd)/daemon/.venv/bin/python cargo build --manifest-path daemon/Cargo.toml --bin brain-daemon
	rm -f ./brain-daemon
	cp target/debug/brain-daemon ./brain-daemon


# Stop running daemon if active, then start a new instance
restart-daemon:
	@echo "Restarting daemon..."
	-./brain-daemon daemon stop || true
	./brain-daemon daemon start

# Run UDS IPC integration and schema conformance tests
test-ipc:
	@echo "Running UDS IPC integration and schema conformance tests..."
	daemon/.venv/bin/pytest daemon/tests/test_schema_requests.py \
		daemon/tests/test_schema_responses.py \
		daemon/tests/test_schema_streams.py \
		daemon/tests/test_uds_ipc.py


# Full dev workflow: build, replace binary, restart daemon, and run tests
dev: build-brain restart-daemon test-ipc


