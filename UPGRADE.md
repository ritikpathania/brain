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

## 1. Upgrading via Cargo

Pull the latest source updates and re-run cargo install:
```bash
cd brain-engine
git pull
PYO3_PYTHON=$(which python3) cargo install --path apps/brain --force --bin brain
```

---

## 2. Reset / Clean-up
If you wish to reset the engine databases:

1. **Clear the databases & configurations**:
   ```bash
   # Reset all graphs and analytics databases
   rm -rf ~/.brain/
   ```

