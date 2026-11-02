"""Product manifest assembly, hash validation, and inventory verification."""

import argparse
import hashlib
import json
import os
import sys
from pathlib import Path, PurePosixPath

# Ensure local imports work
sys.path.insert(0, str(Path(__file__).resolve().parent))
from artifact_manifest import validate_frontend
from runtime_manifest import validate_runtime_manifest


class ProductManifestError(ValueError):
    """Product manifest or assembly is invalid."""


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(64 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def scan_relative_files(root: Path) -> list[str]:
    files = []
    canonical_root = root.resolve(strict=True)
    for dirpath, dirnames, filenames in os.walk(canonical_root):
        dirnames.sort()
        rel_dir = Path(dirpath).relative_to(canonical_root)
        for name in sorted(filenames):
            if name == ".DS_Store":
                continue
            rel_path = (rel_dir / name) if str(rel_dir) != "." else Path(name)
            posix_path = str(PurePosixPath(rel_path))
            files.append(posix_path)
    files.sort()
    return files


def assemble_product_manifest(
    root: Path,
    source: dict,
    target: str,
    build_tools: dict,
) -> dict:
    canonical_root = root.resolve(strict=True)

    cli_path = canonical_root / "bin" / "brain"
    if not cli_path.is_file():
        raise ProductManifestError(f"missing CLI binary: {cli_path}")

    daemon_path = canonical_root / "bin" / "brain-daemon"
    if not daemon_path.is_file():
        raise ProductManifestError(f"missing daemon binary: {daemon_path}")

    fe_manifest_path = canonical_root / "frontend" / "brain-frontend" / "manifest.json"
    if not fe_manifest_path.is_file():
        raise ProductManifestError(f"missing frontend manifest: {fe_manifest_path}")
    fe_manifest = json.loads(fe_manifest_path.read_text())

    d_manifest_path = canonical_root / "runtime-manifest.json"
    if not d_manifest_path.is_file():
        raise ProductManifestError(f"missing daemon manifest: {d_manifest_path}")
    d_manifest = json.loads(d_manifest_path.read_text())

    p_manifest_path = canonical_root / "runtime" / "python-files.json"
    if not p_manifest_path.is_file():
        raise ProductManifestError(f"missing python files manifest: {p_manifest_path}")

    brain_version = fe_manifest.get("brain_version")
    if not brain_version or d_manifest.get("brain_version") != brain_version:
        d_ver = d_manifest.get("brain_version")
        raise ProductManifestError(
            f"version mismatch: frontend ({brain_version}) != daemon ({d_ver})"
        )

    fe_target = fe_manifest.get("target_triple")
    d_target = d_manifest.get("target_triple")
    if fe_target != target or d_target != target:
        raise ProductManifestError(
            f"target mismatch: expected {target}, fe={fe_target}, daemon={d_target}"
        )

    lic_dir = canonical_root / "licenses"
    licenses = []
    if lic_dir.is_dir():
        for dirpath, _, filenames in os.walk(lic_dir):
            for name in sorted(filenames):
                if name == ".DS_Store":
                    continue
                file_path = Path(dirpath) / name
                rel_path = str(PurePosixPath(file_path.relative_to(canonical_root)))
                licenses.append({"path": rel_path, "sha256": digest(file_path)})
    licenses.sort(key=lambda item: item["path"])

    # Determine contents list, including product-manifest.json itself
    current_files = scan_relative_files(canonical_root)
    if "product-manifest.json" not in current_files:
        current_files.append("product-manifest.json")
    current_files.sort()

    manifest = {
        "schema_version": 1,
        "artifact": "brain-product",
        "brain_version": brain_version,
        "source": source,
        "target_triple": target,
        "cli": {
            "path": "bin/brain",
            "sha256": digest(cli_path),
        },
        "frontend": {
            "manifest": "frontend/brain-frontend/manifest.json",
            "sha256": digest(fe_manifest_path),
        },
        "daemon": {
            "manifest": "runtime-manifest.json",
            "sha256": digest(d_manifest_path),
        },
        "python_files": {
            "manifest": "runtime/python-files.json",
            "sha256": digest(p_manifest_path),
        },
        "licenses": licenses,
        "build_tools": build_tools,
        "contents": current_files,
    }

    manifest_file = canonical_root / "product-manifest.json"
    manifest_file.write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def validate_product(root: Path, target: str) -> dict:
    canonical_root = root.resolve(strict=True)
    manifest_path = canonical_root / "product-manifest.json"
    if not manifest_path.is_file():
        raise ProductManifestError(f"product manifest missing: {manifest_path}")

    try:
        manifest = json.loads(manifest_path.read_text())
    except Exception as e:
        raise ProductManifestError(f"invalid JSON in product manifest: {e}") from e

    if (
        manifest.get("schema_version") != 1
        or manifest.get("artifact") != "brain-product"
    ):
        raise ProductManifestError(
            "invalid product manifest schema or artifact declaration"
        )

    if manifest.get("target_triple") != target:
        raise ProductManifestError(
            f"target mismatch: expected {target}, got {manifest.get('target_triple')}"
        )

    brain_version = manifest.get("brain_version")
    if not brain_version:
        raise ProductManifestError("manifest missing brain_version")

    # Validate CLI
    cli_info = manifest.get("cli", {})
    cli_rel = cli_info.get("path")
    if cli_rel != "bin/brain":
        raise ProductManifestError(f"unexpected CLI path: {cli_rel}")
    cli_path = canonical_root / cli_rel
    if not cli_path.is_file() or digest(cli_path) != cli_info.get("sha256"):
        raise ProductManifestError("CLI binary missing or digest mismatch")
    if not os.access(cli_path, os.X_OK):
        raise ProductManifestError("CLI binary not executable")

    # Validate Frontend component
    fe_info = manifest.get("frontend", {})
    fe_rel = fe_info.get("manifest")
    if fe_rel != "frontend/brain-frontend/manifest.json":
        raise ProductManifestError(f"unexpected frontend manifest path: {fe_rel}")
    fe_manifest_path = canonical_root / fe_rel
    if not fe_manifest_path.is_file() or digest(fe_manifest_path) != fe_info.get(
        "sha256"
    ):
        raise ProductManifestError("frontend manifest missing or digest mismatch")

    # Validate Daemon component
    d_info = manifest.get("daemon", {})
    d_rel = d_info.get("manifest")
    if d_rel != "runtime-manifest.json":
        raise ProductManifestError(f"unexpected daemon manifest path: {d_rel}")
    d_manifest_path = canonical_root / d_rel
    if not d_manifest_path.is_file() or digest(d_manifest_path) != d_info.get("sha256"):
        raise ProductManifestError("daemon manifest missing or digest mismatch")

    # Validate Python files manifest
    p_info = manifest.get("python_files", {})
    p_rel = p_info.get("manifest")
    if p_rel != "runtime/python-files.json":
        raise ProductManifestError(f"unexpected python files manifest path: {p_rel}")
    p_manifest_path = canonical_root / p_rel
    if not p_manifest_path.is_file() or digest(p_manifest_path) != p_info.get("sha256"):
        raise ProductManifestError("python files manifest missing or digest mismatch")

    # Validate licenses
    licenses = manifest.get("licenses", [])
    if not isinstance(licenses, list):
        raise ProductManifestError("licenses must be a list")
    for item in licenses:
        item_path_rel = item.get("path")
        if not item_path_rel or not item_path_rel.startswith("licenses/"):
            raise ProductManifestError(f"invalid license path: {item_path_rel}")
        file_path = canonical_root / item_path_rel
        if not file_path.is_file() or digest(file_path) != item.get("sha256"):
            raise ProductManifestError(
                f"license file missing or digest mismatch: {item_path_rel}"
            )

    # Validate closed inventory
    declared_contents = set(manifest.get("contents", []))
    actual_contents = set(scan_relative_files(canonical_root))
    if declared_contents != actual_contents:
        extra = actual_contents - declared_contents
        missing = declared_contents - actual_contents
        raise ProductManifestError(
            f"closed content inventory mismatch; extra={extra}, missing={missing}"
        )

    # Check for forbidden substrings
    forbidden = ["node_modules", ".git", ".venv", "__pycache__", ".pytest_cache"]
    for path_str in actual_contents:
        parts = path_str.split("/")
        if any(f in parts for f in forbidden):
            raise ProductManifestError(f"forbidden content path in product: {path_str}")

    # Recursive component validations
    try:
        validate_frontend(
            canonical_root / "frontend" / "brain-frontend", brain_version, target
        )
    except Exception as e:
        raise ProductManifestError(f"frontend validation failed: {e}") from e

    try:
        validate_runtime_manifest(canonical_root)
    except Exception as e:
        raise ProductManifestError(f"runtime manifest validation failed: {e}") from e

    # Validate python files integrity
    p_data = json.loads(p_manifest_path.read_text())
    for entry in p_data.get("files", []):
        entry_path = canonical_root / "runtime" / "python" / entry["path"]
        if entry["type"] == "file":
            if not entry_path.is_file() or digest(entry_path) != entry["sha256"]:
                raise ProductManifestError(
                    f"python leaf file missing or digest mismatch: {entry['path']}"
                )
        elif entry["type"] == "symlink":
            if (
                not entry_path.is_symlink()
                or os.readlink(entry_path) != entry["target"]
            ):
                raise ProductManifestError(f"python symlink mismatch: {entry['path']}")

    return manifest


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Product manifest assembler and verifier"
    )
    parser.add_argument(
        "--artifact", required=True, type=Path, help="Product artifact directory"
    )
    parser.add_argument("--target", required=True, help="Target triple")
    parser.add_argument(
        "--assemble", action="store_true", help="Assemble product manifest"
    )
    parser.add_argument("--source", type=Path, help="Path to source identity JSON")
    parser.add_argument("--build-tools", type=Path, help="Path to build tools JSON")

    args = parser.parse_args()

    try:
        if args.assemble:
            if not args.source or not args.build_tools:
                raise ProductManifestError(
                    "--assemble requires --source and --build-tools"
                )
            source_data = json.loads(args.source.read_text())
            tools_data = json.loads(args.build_tools.read_text())
            assemble_product_manifest(
                args.artifact, source_data, args.target, tools_data
            )
            manifest_file = args.artifact / "product-manifest.json"
            print(f"Product manifest assembled at {manifest_file}")
        else:
            validate_product(args.artifact, args.target)
            print("Product validation passed")
    except Exception as e:
        sys.stderr.write(f"error: {e}\n")
        sys.exit(1)
