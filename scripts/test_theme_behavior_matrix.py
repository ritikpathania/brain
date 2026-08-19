#!/usr/bin/env python3
"""
Comprehensive Theme Behavior Matrix Verification
Delegates to the Canonical Claude Oracle Parity Engine (scripts/oracle_parity_engine.py)
to enforce exact-grid differential behavioral verification across NORMAL and VIM modes.
"""

import sys
from oracle_parity_engine import main as run_engine_main

if __name__ == "__main__":
    sys.exit(run_engine_main())
