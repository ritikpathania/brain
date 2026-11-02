"""Each shipped binary/config and native loader must reject host dependencies."""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from host_paths import HostPathError, scan_host_paths, validate_loader_output


class HostPathTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_all_payload_classes_and_forbidden_paths(self):
        for name in [
            "bin/brain",
            "bin/brain-daemon",
            "brain-frontend",
            "manifest.json",
        ]:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            for forbidden in [
                b"/Users/builder/a",
                b"/home/builder/a",
                b"/build/brain",
                b"/elsewhere/claude-code/source",
                b"/ref/claude-analysis/a",
                b"/cache/uv/python/build/lib.so",
                b"/cache/uv/builds-v0/a",
            ]:
                with self.subTest(name=name, forbidden=forbidden):
                    path.write_bytes(b"\x00prefix" + forbidden + b"\x00suffix")
                    with self.assertRaises(HostPathError):
                        scan_host_paths(self.root, Path("/build/brain"), [])
            path.unlink()

    def test_system_paths_and_relative_artifact_paths_allowed(self):
        (self.root / "manifest.json").write_text('"runtime/python/lib"')
        scan_host_paths(self.root, Path("/build/brain"), [])
        validate_loader_output(
            "/usr/lib/libSystem.B.dylib\n@loader_path/../runtime/python/lib",
            self.root,
            "darwin",
        )

    def test_loader_absolute_host_path_rejected(self):
        for text in ["/Users/b/lib.dylib", "/home/b/lib.so", "/opt/foreign/lib.dylib"]:
            with self.subTest(text=text), self.assertRaises(HostPathError):
                validate_loader_output(text, self.root, "darwin")
        with self.assertRaises(HostPathError):
            validate_loader_output("libpython.so => not found", self.root, "linux")

    def test_windows_doc_path_allowed(self):
        (self.root / "doc.txt").write_bytes(
            b'# On Windows path is like "/C:/Users/foo"'
        )
        scan_host_paths(self.root, Path("/build/brain"), [])

    def test_read_error_and_escaping_link_not_masked(self):
        missing = self.root / "missing"
        (self.root / "escape").symlink_to(missing)
        with self.assertRaises(HostPathError):
            scan_host_paths(self.root, Path("/build/brain"), [])

    def test_file_read_error_propagates(self):
        (self.root / "binary").write_bytes(b"safe")
        with patch.object(Path, "open", side_effect=PermissionError("fixture")):
            with self.assertRaises(HostPathError):
                scan_host_paths(self.root, Path("/build/brain"), [])


if __name__ == "__main__":
    unittest.main()
