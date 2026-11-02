"""Tests for product manifest assembly, hash graph validation, and closed inventory."""

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

# Ensure scripts/release is on sys.path
sys.path.insert(0, str(Path(__file__).resolve().parent))

try:
    from product_manifest import (
        ProductManifestError,
        assemble_product_manifest,
        validate_product,
    )
except ImportError:
    ProductManifestError = None
    assemble_product_manifest = None
    validate_product = None


def sha256_digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(64 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


class ProductManifestTests(unittest.TestCase):
    def setUp(self):
        if assemble_product_manifest is None or validate_product is None:
            self.fail("product_manifest module not yet implemented")

        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name) / "product"
        self.root.mkdir()

        self.target = "aarch64-apple-darwin"
        self.brain_version = "1.1.0"
        self.source = {
            "sha": "a" * 40,
            "dirty": False,
            "fingerprint_algorithm": "brain-source-tree-v1",
            "fingerprint_sha256": "b" * 64,
            "untracked_input_count": 0,
        }
        self.build_tools = {
            "rustc": "rustc 1.99.1",
            "cargo": "cargo 1.99.1",
            "bun": "1.4.4",
            "python": "3.15.2",
        }

        # Build fixture layout
        self.create_fixture_product()

    def create_fixture_product(self):
        bin_dir = self.root / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        cli_bin = bin_dir / "brain"
        cli_bin.write_bytes(b"mock-cli-binary")
        cli_bin.chmod(0o755)

        daemon_bin = bin_dir / "brain-daemon"
        daemon_bin.write_bytes(b"mock-daemon-binary")
        daemon_bin.chmod(0o755)

        fe_dir = self.root / "frontend" / "brain-frontend"
        fe_dir.mkdir(parents=True, exist_ok=True)
        fe_bin = fe_dir / "brain-frontend"

        identity = {
            "schema_version": 1,
            "identity": "brain-frontend",
            "package": "@brain/frontend",
            "frontend_version": "0.1.0",
            "brain_version": self.brain_version,
            "source": self.source,
            "target_triple": self.target,
            "runtime_model": "bun-standalone",
            "bun_version": "1.4.4",
            "lockfile_sha256": "c" * 64,
        }
        encoded = json.dumps(identity, separators=(",", ":"))
        fe_bin.write_text("#!/bin/sh\nprintf '%s\\n' '" + encoded + "'\n")
        fe_bin.chmod(0o755)

        fe_manifest = dict(identity)
        fe_manifest["entrypoint"] = "brain-frontend"
        fe_manifest["executable_sha256"] = sha256_digest(fe_bin)
        (fe_dir / "manifest.json").write_text(json.dumps(fe_manifest, indent=2))

        py_home = self.root / "runtime" / "python"
        lib_path = py_home / "lib" / "libpython3.15.dylib"
        lib_path.parent.mkdir(parents=True, exist_ok=True)
        lib_path.write_bytes(b"mock-libpython")
        lib_sha = sha256_digest(lib_path)

        p_manifest = {
            "schema_version": 1,
            "files": [
                {"path": "lib/libpython3.15.dylib", "type": "file", "sha256": lib_sha}
            ],
        }
        (self.root / "runtime" / "python-files.json").write_text(
            json.dumps(p_manifest, indent=2)
        )

        d_manifest = {
            "schema_version": 1,
            "artifact": "brain-daemon-runtime",
            "brain_version": self.brain_version,
            "daemon_source_sha": self.source["sha"],
            "source_tree": {"dirty": False, "untracked_file_count": 0},
            "target_triple": self.target,
            "rust_version": "rustc 1.99.1",
            "python": {
                "version": "3.15.2",
                "home": "runtime/python",
                "library": "lib/libpython3.15.dylib",
                "library_sha256": lib_sha,
            },
            "daemon_sha256": sha256_digest(daemon_bin),
            "runtime_files_manifest": "runtime/python-files.json",
            "contents": [
                "bin/brain-daemon",
                "runtime/python",
                "runtime-manifest.json",
                "runtime/python-files.json",
            ],
        }
        (self.root / "runtime-manifest.json").write_text(
            json.dumps(d_manifest, indent=2)
        )

        lic_dir = self.root / "licenses"
        lic_dir.mkdir(parents=True, exist_ok=True)
        (lic_dir / "Brain-LICENSE.txt").write_text("Brain License Text")
        (lic_dir / "notice-inventory.json").write_text(
            json.dumps({"schema_version": 1, "items": []}, indent=2)
        )

        assemble_product_manifest(
            root=self.root,
            source=self.source,
            target=self.target,
            build_tools=self.build_tools,
        )

    def test_assemble_and_validate_roundtrip(self):
        manifest = validate_product(self.root, self.target)
        self.assertEqual(manifest["schema_version"], 1)
        self.assertEqual(manifest["artifact"], "brain-product")
        self.assertEqual(manifest["brain_version"], self.brain_version)
        self.assertEqual(manifest["target_triple"], self.target)
        self.assertEqual(manifest["cli"]["path"], "bin/brain")

    def test_python_inventory_tampering_is_rejected(self):
        p_path = self.root / "runtime" / "python-files.json"
        data = json.loads(p_path.read_text())
        data["files"].append(
            {"path": "lib/extra.py", "type": "file", "sha256": "0" * 64}
        )
        p_path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_python_leaf_tampering_is_rejected(self):
        lib_path = self.root / "runtime" / "python" / "lib" / "libpython3.15.dylib"
        lib_path.write_bytes(b"tampered-content")
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_cli_tampering_rejected(self):
        (self.root / "bin" / "brain").write_bytes(b"tampered-cli")
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_frontend_manifest_tampering_rejected(self):
        f_path = self.root / "frontend" / "brain-frontend" / "manifest.json"
        data = json.loads(f_path.read_text())
        data["frontend_version"] = "9.9.9"
        f_path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_daemon_manifest_tampering_rejected(self):
        d_path = self.root / "runtime-manifest.json"
        data = json.loads(d_path.read_text())
        data["rust_version"] = "corrupted"
        d_path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_unexpected_extra_file_fails_closed_inventory(self):
        (self.root / "unexpected.txt").write_text("foreign file")
        with self.assertRaises(ValueError):
            validate_product(self.root, self.target)

    def test_target_mismatch_rejected(self):
        with self.assertRaises(ValueError):
            validate_product(self.root, "x86_64-unknown-linux-gnu")


if __name__ == "__main__":
    unittest.main()
