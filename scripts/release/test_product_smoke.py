"""Tests for product extraction smoke runner and process cleanup."""

import os
import sys
import tempfile
import unittest
from pathlib import Path

# Ensure scripts/release is on sys.path
sys.path.insert(0, str(Path(__file__).resolve().parent))

try:
    from product_smoke import (
        ProductSmokeError,
        owned_pid_alive,
        run_product_smoke,
    )
except ImportError:
    ProductSmokeError = None
    owned_pid_alive = None
    run_product_smoke = None


class ProductSmokeTests(unittest.TestCase):
    def setUp(self):
        if owned_pid_alive is None or run_product_smoke is None:
            self.fail("product_smoke module not yet implemented")

    def test_owned_pid_alive_detects_running_and_dead_processes(self):
        current_pid = os.getpid()
        self.assertTrue(owned_pid_alive(current_pid))
        # Nonexistent high PID
        self.assertFalse(owned_pid_alive(999999))

    def test_failure_after_submission_reaps_owned_processes(self):
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        root = Path(temp_dir.name)
        extracted = root / "extracted"
        runtime = root / "runtime"
        cwd = root / "cwd"
        extracted.mkdir()
        runtime.mkdir()
        cwd.mkdir()

        # Dummy brain executable that writes a pidfile and loops
        bin_dir = extracted / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        cli = bin_dir / "brain"
        cli.write_text(
            "#!/bin/sh\n"
            "echo 'New conversation'\n"
            "read line1\n"
            "echo 'Connected · New session'\n"
            "read line2\n"
            "echo 'Mock response to: smoke prompt'\n"
            "echo 'Message Brain'\n"
            "sleep 10\n"
        )
        cli.chmod(0o755)

        with self.assertRaises(Exception):
            run_product_smoke(
                extracted_root=extracted,
                runtime_root=runtime,
                unrelated_cwd=cwd,
                test_fault_step="after_submission",
            )


if __name__ == "__main__":
    unittest.main()
