"""Owned schema/containment/probe fixtures; no frontend implementation copies."""

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from artifact_manifest import ArtifactError, resolve_contained, validate_frontend


class FrontendManifestTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / "artifact"
        self.root.mkdir()
        self.identity = {
            "schema_version": 1,
            "identity": "brain-frontend",
            "package": "@brain/frontend",
            "frontend_version": "0.1.0",
            "brain_version": "1.1.0",
            "source": {
                "sha": "a" * 40,
                "dirty": False,
                "fingerprint_algorithm": "brain-source-tree-v1",
                "fingerprint_sha256": "b" * 64,
                "untracked_input_count": 0,
            },
            "target_triple": "aarch64-apple-darwin",
            "runtime_model": "bun-standalone",
            "bun_version": "1.4.4",
            "lockfile_sha256": "c" * 64,
        }
        self.executable = self.root / "brain-frontend"
        self.install_probe(self.identity)
        self.manifest = {
            **self.identity,
            "entrypoint": "brain-frontend",
            "executable_sha256": hashlib.sha256(
                self.executable.read_bytes()
            ).hexdigest(),
        }
        self.save()

    def install_probe(self, identity):
        # Executable fixtures exercise the real bounded subprocess diagnostic.
        encoded = json.dumps(identity, separators=(",", ":"))
        self.executable.write_text("#!/bin/sh\nprintf '%s\\n' '" + encoded + "'\n")
        self.executable.chmod(0o755)

    def save(self):
        (self.root / "manifest.json").write_text(json.dumps(self.manifest))

    def validate(self):
        return validate_frontend(self.root, "1.1.0", "aarch64-apple-darwin")

    def test_valid_complete_identity_and_schema(self):
        self.assertEqual(self.validate(), self.manifest)

    def test_bad_or_missing_fields(self):
        original = copy.deepcopy(self.manifest)
        cases = [
            ("identity", "foreign"),
            ("package", "foreign"),
            ("target_triple", "x86_64-unknown-linux-gnu"),
            ("runtime_model", "bun"),
            ("bun_version", "1.4.2"),
            ("bun_version", "1.4.1"),
            ("brain_version", "1.2.0"),
            ("frontend_version", "latest"),
            ("schema_version", True),
            ("lockfile_sha256", "A" * 64),
            ("executable_sha256", "0" * 64),
            ("entrypoint", "../foreign"),
            ("entrypoint", "/tmp/foreign"),
        ]
        for key, value in cases:
            with self.subTest(key=key, value=value):
                self.manifest = {**original, key: value}
                self.save()
                with self.assertRaises(ArtifactError):
                    self.validate()
        for key in original:
            with self.subTest(missing=key):
                self.manifest = {k: v for k, v in original.items() if k != key}
                self.save()
                with self.assertRaises(ArtifactError):
                    self.validate()

    def test_source_types_and_unknown_fields(self):
        for key, value in [
            ("dirty", "false"),
            ("sha", "a" * 7),
            ("fingerprint_sha256", "z" * 64),
            ("untracked_input_count", True),
            ("untracked_input_count", -1),
            ("fingerprint_algorithm", "foreign"),
        ]:
            with self.subTest(key=key):
                original = self.manifest["source"][key]
                self.manifest["source"][key] = value
                self.save()
                with self.assertRaises(ArtifactError):
                    self.validate()
                self.manifest["source"][key] = original
        self.manifest["other_identity"] = "spoof"
        self.save()
        with self.assertRaises(ArtifactError):
            self.validate()

    def test_duplicate_json_key_rejected(self):
        text = json.dumps(self.manifest)
        (self.root / "manifest.json").write_text(
            text[:-1] + ',"identity":"brain-frontend"}'
        )
        with self.assertRaises(ArtifactError):
            self.validate()

    def test_missing_executable_and_permission(self):
        self.executable.chmod(0o644)
        with self.assertRaises(ArtifactError):
            self.validate()
        self.executable.unlink()
        with self.assertRaises(ArtifactError):
            self.validate()

    def test_containment_and_symlink_escape(self):
        outside = self.root.parent / "foreign"
        outside.write_text("foreign")
        self.executable.unlink()
        self.executable.symlink_to(outside)
        with self.assertRaises(ArtifactError):
            self.validate()
        for path in ["../foreign", "/tmp/foreign", "a/../foreign", "", "a\\foreign"]:
            with self.subTest(path=path), self.assertRaises(ArtifactError):
                resolve_contained(self.root, path)

    def test_compiled_identity_mismatch_cannot_be_spoofed(self):
        self.install_probe({**self.identity, "frontend_version": "0.2.0"})
        self.manifest["executable_sha256"] = hashlib.sha256(
            self.executable.read_bytes()
        ).hexdigest()
        self.save()
        with self.assertRaisesRegex(ArtifactError, "compiled identity"):
            self.validate()

    def test_probe_timeout_and_output_limit(self):
        from artifact_manifest import probe_identity

        self.executable.write_text("#!/bin/sh\nexec /bin/sleep 30\n")
        with self.assertRaisesRegex(ArtifactError, "deadline"):
            probe_identity(self.executable, timeout=0.05)
        self.executable.write_text("#!/bin/sh\nexec /usr/bin/yes noisy\n")
        with self.assertRaisesRegex(ArtifactError, "output"):
            probe_identity(self.executable)


if __name__ == "__main__":
    unittest.main()
