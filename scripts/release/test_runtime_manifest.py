"""Tests for daemon runtime manifest validation and shared library resolution."""

import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Ensure scripts/release is on sys.path for direct execution
sys.path.insert(0, str(Path(__file__).resolve().parent))

# Note: runtime_manifest will be imported once created.
try:
    from runtime_manifest import (
        RuntimeManifestError,
        resolve_python_library,
        resolve_python_version,
        validate_runtime_manifest,
    )
except ImportError:
    RuntimeManifestError = None
    resolve_python_library = None
    resolve_python_version = None
    validate_runtime_manifest = None


def make_dummy_file(path: Path, content: bytes = b"dummy") -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    hasher = hashlib.sha256()
    hasher.update(content)
    return hasher.hexdigest()


class RuntimeManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name) / "artifact"
        self.root.mkdir()

        # Create daemon binary fixture
        self.daemon_sha = make_dummy_file(
            self.root / "bin/brain-daemon", b"daemon-binary"
        )
        (self.root / "bin/brain-daemon").chmod(0o755)

    def create_artifact(self, version: str, lib_rel: str) -> Path:
        py_home = self.root / "runtime/python"
        lib_path = py_home / lib_rel
        lib_sha = make_dummy_file(lib_path, b"libpython-content")

        files_manifest = self.root / "runtime/python-files.json"
        files_data = {
            "schema_version": 1,
            "files": [{"path": lib_rel, "type": "file", "sha256": lib_sha}],
        }
        files_manifest.write_text(json.dumps(files_data))

        manifest = {
            "schema_version": 1,
            "artifact": "brain-daemon-runtime",
            "brain_version": "1.1.0",
            "daemon_source_sha": "a" * 40,
            "source_tree": {"dirty": False, "untracked_file_count": 0},
            "target_triple": "aarch64-apple-darwin",
            "rust_version": "rustc 1.99.1",
            "python": {
                "version": version,
                "home": "runtime/python",
                "library": lib_rel,
                "library_sha256": lib_sha,
            },
            "daemon_sha256": self.daemon_sha,
            "runtime_files_manifest": "runtime/python-files.json",
            "contents": [
                "bin/brain-daemon",
                "runtime/python",
                "runtime-manifest.json",
                "runtime/python-files.json",
            ],
        }
        (self.root / "runtime-manifest.json").write_text(json.dumps(manifest))
        return lib_path

    def test_312_library_path_resolves_from_manifest(self):
        if resolve_python_library is None:
            self.fail("runtime_manifest module not yet implemented")
        expected_lib = self.create_artifact("3.12.13", "lib/libpython3.12.dylib")
        resolved = resolve_python_library(self.root)
        self.assertEqual(resolved, expected_lib.resolve())

    def test_315_library_path_resolves_from_manifest(self):
        if resolve_python_library is None:
            self.fail("runtime_manifest module not yet implemented")
        expected_lib = self.create_artifact("3.15.2", "lib/libpython3.15.dylib")
        resolved = resolve_python_library(self.root)
        self.assertEqual(resolved, expected_lib.resolve())

    def test_linux_library_path_resolves_for_both_versions(self):
        if resolve_python_library is None:
            self.fail("runtime_manifest module not yet implemented")
        for ver, lib_rel in [
            ("3.12.13", "lib/libpython3.12.so.1.0"),
            ("3.15.2", "lib/libpython3.15.so.1.0"),
        ]:
            with self.subTest(version=ver, library=lib_rel):
                expected_lib = self.create_artifact(ver, lib_rel)
                resolved = resolve_python_library(self.root)
                self.assertEqual(resolved, expected_lib.resolve())

    def test_reject_traversal_or_escaping_path(self):
        if resolve_python_library is None:
            self.fail("runtime_manifest module not yet implemented")
        self.create_artifact("3.15.2", "lib/libpython3.15.dylib")
        manifest_path = self.root / "runtime-manifest.json"
        data = json.loads(manifest_path.read_text())
        data["python"]["library"] = "../../../etc/passwd"
        manifest_path.write_text(json.dumps(data))
        with self.assertRaises(RuntimeManifestError):
            resolve_python_library(self.root)

    def test_verify_script_rejects_legacy_312_manifest(self):
        self.create_artifact("3.12.13", "lib/libpython3.12.dylib")
        script = Path(__file__).resolve().parent.parent / "verify-daemon-runtime.sh"
        res = subprocess.run(
            [str(script), "--artifact", str(self.root)], capture_output=True, text=True
        )
        self.assertNotEqual(res.returncode, 0)
        self.assertTrue(
            "artifact does not declare CPython 3.15" in res.stderr
            or "artifact does not declare CPython 3.15" in res.stdout
        )

    def test_cli_invocation_emits_single_absolute_path(self):
        expected_lib = self.create_artifact("3.15.2", "lib/libpython3.15.dylib")
        cli = Path(__file__).resolve().parent / "runtime_manifest.py"
        res = subprocess.run(
            [sys.executable, str(cli), "--artifact", str(self.root)],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, res.stderr)
        self.assertEqual(res.stdout.strip(), str(expected_lib.resolve()))

    def test_cli_invocation_fails_on_invalid_artifact(self):
        cli = Path(__file__).resolve().parent / "runtime_manifest.py"
        res = subprocess.run(
            [sys.executable, str(cli), "--artifact", str(self.root / "nonexistent")],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(res.returncode, 0)
        self.assertIn("error:", res.stderr)


if __name__ == "__main__":
    unittest.main()
