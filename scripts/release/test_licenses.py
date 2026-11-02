"""Tests for license notice collection and inventory validation."""

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

# Ensure scripts/release is on sys.path
sys.path.insert(0, str(Path(__file__).resolve().parent))

try:
    from licenses import LicenseError, collect_notices
except ImportError:
    LicenseError = None
    collect_notices = None


def sha256_digest(content: bytes) -> str:
    hasher = hashlib.sha256()
    hasher.update(content)
    return hasher.hexdigest()


class LicenseTests(unittest.TestCase):
    def setUp(self):
        if collect_notices is None:
            self.fail("licenses module not yet implemented")

        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.output_dir = Path(self.temp_dir.name) / "licenses"
        self.output_dir.mkdir()

        self.notice_src = Path(self.temp_dir.name) / "test-license.txt"
        self.notice_bytes = b"Sample MIT License Notice"
        self.notice_src.write_bytes(self.notice_bytes)
        self.notice_sha = sha256_digest(self.notice_bytes)

    def test_collect_notices_success(self):
        inputs = [
            {
                "name": "sample-pkg",
                "version": "1.0.0",
                "notice_source": str(self.notice_src),
                "notice_sha256": self.notice_sha,
                "output_filename": "sample-LICENSE.txt",
            }
        ]
        result = collect_notices(inputs, self.output_dir)
        self.assertEqual(len(result), 2)  # Notice + notice-inventory.json

        written_notice = self.output_dir / "sample-LICENSE.txt"
        self.assertTrue(written_notice.is_file())
        self.assertEqual(written_notice.read_bytes(), self.notice_bytes)

        inventory_file = self.output_dir / "notice-inventory.json"
        self.assertTrue(inventory_file.is_file())
        inventory = json.loads(inventory_file.read_text())
        self.assertEqual(inventory.get("schema_version"), 1)

    def test_missing_notice_file_fails(self):
        inputs = [
            {
                "name": "missing-pkg",
                "version": "1.0.0",
                "notice_source": str(Path(self.temp_dir.name) / "nonexistent.txt"),
                "notice_sha256": "0" * 64,
                "output_filename": "missing.txt",
            }
        ]
        with self.assertRaises(ValueError):
            collect_notices(inputs, self.output_dir)

    def test_sha_mismatch_fails(self):
        inputs = [
            {
                "name": "mismatch-pkg",
                "version": "1.0.0",
                "notice_source": str(self.notice_src),
                "notice_sha256": "0" * 64,
                "output_filename": "mismatch.txt",
            }
        ]
        with self.assertRaises(ValueError):
            collect_notices(inputs, self.output_dir)


if __name__ == "__main__":
    unittest.main()
