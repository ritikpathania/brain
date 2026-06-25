# brain Upgrade & Maintenance Guide

This document details how to upgrade, configure, and clean up the `brain` developer tool.

---

## State & Data Maintenance
All tool data, logs, and sockets are isolated under the user's home directory at `~/.brain/`.
- **Database (SQLite)**: `~/.brain/memory.db`
- **Analytics (DuckDB)**: `~/.brain/analytics.duckdb`
- **Daemon Socket**: `~/.brain/daemon.sock`
- **Daemon PID File**: `~/.brain/daemon.pid`
- **Daemon Logs**: `~/.brain/daemon.log`

---

## 1. Upgrading using the Toolchains

### Cargo Upgrade
Pull the latest source updates and re-run cargo install:
```bash
cd brain-engine
git pull
PYO3_PYTHON=$(which python3) cargo install --path daemon --force --bin brain
```

### UV Tool Upgrade
Use `uv` to force reinstall:
```bash
uv tool install ./daemon --force
```

### Bun/NPM Upgrade
Re-run global install:
```bash
cd cli
git pull
bun install -g
```

---

## 2. Relinking / Clean-up
If you encounter runtime path clashes, stale sockets, or wish to reset the engine databases:

1. **Stop the daemon**:
   ```bash
   brain daemon stop
   ```
2. **Clear the databases & configurations**:
   ```bash
   # Reset all graphs and analytics databases
   rm -rf ~/.brain/
   ```
3. **Check daemon status**:
   ```bash
   brain daemon status
   ```
