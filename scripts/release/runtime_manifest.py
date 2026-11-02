"""Relocatable daemon runtime manifest validation and shared library resolution."""

import hashlib
import json
from pathlib import Path, PurePosixPath


class RuntimeManifestError(ValueError):
    """A runtime manifest is invalid or missing required declarations."""


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def validate_runtime_manifest(root: Path) -> dict:
    manifest_path = root / "runtime-manifest.json"
    if not manifest_path.is_file():
        raise RuntimeManifestError(f"runtime manifest missing: {manifest_path}")

    try:
        manifest = json.loads(manifest_path.read_text())
    except (OSError, json.JSONDecodeError) as e:
        raise RuntimeManifestError(f"invalid JSON in runtime manifest: {e}") from e

    if (
        manifest.get("schema_version") != 1
        or manifest.get("artifact") != "brain-daemon-runtime"
    ):
        raise RuntimeManifestError(
            "invalid daemon runtime manifest schema or artifact identity"
        )

    python_section = manifest.get("python")
    if not isinstance(python_section, dict):
        raise RuntimeManifestError("manifest missing 'python' configuration section")

    version = python_section.get("version")
    if not isinstance(version, str) or not version:
        raise RuntimeManifestError("manifest python.version must be a non-empty string")

    home = python_section.get("home")
    if home != "runtime/python":
        raise RuntimeManifestError(
            f"unexpected python.home: {home}, expected 'runtime/python'"
        )

    library_rel = python_section.get("library")
    if not isinstance(library_rel, str) or not library_rel or "\\" in library_rel:
        raise RuntimeManifestError(
            "python.library must be a non-empty relative POSIX path"
        )

    posix_path = PurePosixPath(library_rel)
    if posix_path.is_absolute() or any(
        p in {"", ".", ".."} for p in library_rel.split("/")
    ):
        raise RuntimeManifestError(
            f"python.library must not be absolute or traverse: {library_rel}"
        )

    return manifest


def resolve_python_library(root: Path) -> Path:
    manifest = validate_runtime_manifest(root)
    py_home_rel = manifest["python"]["home"]
    library_rel = manifest["python"]["library"]

    canonical_root = root.resolve(strict=True)
    py_home = (canonical_root / py_home_rel).resolve(strict=True)
    if not py_home.is_relative_to(canonical_root):
        raise RuntimeManifestError("python.home escapes artifact root")

    lib_path = (py_home / library_rel).resolve(strict=True)
    if not lib_path.is_relative_to(py_home):
        raise RuntimeManifestError(f"python.library escapes python home: {library_rel}")

    if not lib_path.is_file():
        raise RuntimeManifestError(f"declared python.library not found: {lib_path}")

    return lib_path


def resolve_python_version(root: Path) -> str:
    manifest = validate_runtime_manifest(root)
    return manifest["python"]["version"]


if __name__ == "__main__":
    import argparse
    import sys

    parser = argparse.ArgumentParser(
        description="Resolve CPython shared library from runtime manifest"
    )
    parser.add_argument(
        "--artifact", required=True, type=Path, help="Artifact root directory"
    )
    args = parser.parse_args()

    try:
        lib_path = resolve_python_library(args.artifact)
        print(str(lib_path))
    except Exception as e:
        sys.stderr.write(f"error: {e}\n")
        sys.exit(1)
