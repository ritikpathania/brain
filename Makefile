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

