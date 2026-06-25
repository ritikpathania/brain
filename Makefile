.PHONY: setup build-bridge build-daemon run-daemon run-cli lint format type-check

# Setup all dependencies and environments
setup:
	@echo "Installing CLI dependencies..."
	cd cli && bun install
	@echo "Syncing daemon virtual environment..."
	cd daemon && uv sync

# Compile the PyO3 Rust extension module for Python
build-bridge:
	cd daemon && uv run maturin develop

# Compile the Rust daemon binary
build-daemon:
	cd daemon && PYO3_PYTHON=$(shell pwd)/daemon/.venv/bin/python cargo build --bin brain

# Start the background Rust IPC socket daemon
run-daemon: build-daemon
	cd daemon && PYO3_PYTHON=$$(pwd)/.venv/bin/python cargo run --bin brain daemon run

# Start the interactive Bun Ink TUI client
run-cli:
	cd cli && bun run src/main.tsx

# Lint python code using Ruff
lint:
	cd daemon && uv run ruff check .

# Format python code using Ruff
format:
	cd daemon && uv run ruff format .

# Type-check python code using ty and pyrefly
type-check:
	cd daemon && uv run ty check && uv run pyrefly check
