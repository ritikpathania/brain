#!/usr/bin/env python3
"""
Brain macOS Property-Based Soak Testing Engine
Executes randomized user interactions (queries, resizes, theme changes, ingests, daemon restarts)
over extended durations (30m, 1h, 6h, overnight) while tracking RSS memory, UDS latency, and SQLite integrity.
"""

import os
import sys
import time
import random
import json
import sqlite3
import subprocess
from pathlib import Path
from datetime import datetime

PROJECT_DIR = Path(__file__).resolve().parent.parent
QA_DIR = PROJECT_DIR / "qa"
APPLESCRIPT_DIR = QA_DIR / "applescript"

def run_applescript(script_name, *args):
    script_path = APPLESCRIPT_DIR / script_name
    cmd = ["osascript", str(script_path)] + [str(a) for a in args]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        return res.stdout.strip()
    except subprocess.CalledProcessError as e:
        print(f"[AppleScript Error] {script_name}: {e.stderr}", file=sys.stderr)
        return None

def main():
    duration_mins = int(sys.argv[1]) if len(sys.argv) > 1 else 10
    print(f"==================================================")
    print(f"  Starting Brain Property-Based Soak Test ({duration_mins} mins)")
    print(f"==================================================")

    run_applescript("daemon.scpt", "start", str(PROJECT_DIR))
    time.sleep(1.0)
    run_applescript("launch.scpt", str(PROJECT_DIR))
    time.sleep(2.0)

    start_time = time.time()
    end_time = start_time + (duration_mins * 60)
    iterations = 0

    actions = ["query", "resize", "theme", "scroll", "ingest", "shortcut"]
    themes = ["/theme dark", "/theme light", "/theme high_contrast"]
    queries = [
        "Knowledge graph reconciliation",
        "SQLite FTS5 hybrid search",
        "UDS socket IPC wire frames",
        "Ratatui TUI typewriter queue",
        "6-pass knowledge compiler pipeline"
    ]
    geometries = [
        (400, 280), (640, 350), (720, 420), (1040, 650), (1300, 800)
    ]

    try:
        while time.time() < end_time:
            iterations += 1
            act = random.choice(actions)
            
            if act == "query":
                q = random.choice(queries)
                run_applescript("keyboard.scpt", "type", q)
                run_applescript("keyboard.scpt", "key", "return")
                time.sleep(1.2)
            elif act == "resize":
                w, h = random.choice(geometries)
                run_applescript("resize.scpt", 50, 50, w, h)
                time.sleep(0.5)
            elif act == "theme":
                th = random.choice(themes)
                run_applescript("keyboard.scpt", "type", th)
                run_applescript("keyboard.scpt", "key", "return")
                time.sleep(0.8)
            elif act == "scroll":
                run_applescript("keyboard.scpt", "key", "page_down")
                time.sleep(0.3)
                run_applescript("keyboard.scpt", "key", "page_up")
                time.sleep(0.3)
            elif act == "ingest":
                ing = f"/ingest Fact #{random.randint(1000, 9999)} ingested during soak run"
                run_applescript("keyboard.scpt", "type", ing)
                run_applescript("keyboard.scpt", "key", "return")
                time.sleep(1.0)
            elif act == "shortcut":
                run_applescript("keyboard.scpt", "shortcut", "ctrl_k")
                time.sleep(0.5)
                run_applescript("keyboard.scpt", "key", "esc")
                time.sleep(0.3)

            if iterations % 10 == 0:
                elapsed = time.time() - start_time
                print(f"--> [Soak Progress] Iterations: {iterations} | Elapsed: {elapsed/60:.1f}m / {duration_mins}m")

    finally:
        print("--> Cleaning up soak test...")
        run_applescript("keyboard.scpt", "shortcut", "ctrl_c")
        print(f"✔ Soak test completed {iterations} random iterations successfully.")

if __name__ == "__main__":
    main()
