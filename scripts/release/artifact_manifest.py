"""Strict first-party frontend artifact validation; build-host tooling only."""

import argparse
import hashlib
import json
import os
import re
import selectors
import signal
import subprocess
import time
from pathlib import Path, PurePosixPath


class ArtifactError(ValueError):
    """An artifact is incomplete, incompatible or unverifiable."""


IDENTITY_FIELDS = {
    "schema_version",
    "identity",
    "package",
    "frontend_version",
    "brain_version",
    "source",
    "target_triple",
    "runtime_model",
    "bun_version",
    "lockfile_sha256",
}
SOURCE_FIELDS = {
    "sha",
    "dirty",
    "fingerprint_algorithm",
    "fingerprint_sha256",
    "untracked_input_count",
}
TARGETS = {"aarch64-apple-darwin", "x86_64-unknown-linux-gnu"}
SEMVER = re.compile(
    r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


def resolve_contained(root: Path, relative: str) -> Path:
    if (
        not isinstance(relative, str)
        or not relative
        or "\\" in relative
        or "\x00" in relative
    ):
        raise ArtifactError("artifact path must be a nonempty POSIX relative path")
    path = PurePosixPath(relative)
    if path.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.split("/")
    ):
        raise ArtifactError(
            "artifact path must not be absolute or traverse directories"
        )
    try:
        canonical_root = root.resolve(strict=True)
        resolved = (canonical_root / relative).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise ArtifactError(f"missing or invalid artifact path: {relative}") from error
    if not resolved.is_relative_to(canonical_root):
        raise ArtifactError(f"artifact symlink escapes containment: {relative}")
    return resolved


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                hasher.update(chunk)
    except OSError as error:
        raise ArtifactError("cannot read artifact for digest verification") from error
    return hasher.hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ArtifactError(f"duplicate manifest key: {key}")
        result[key] = value
    return result


def read_json(path: Path, limit: int) -> dict:
    try:
        with path.open("rb") as handle:
            encoded = handle.read(limit + 1)
        if len(encoded) > limit:
            raise ArtifactError("manifest exceeds size limit")
        value = json.loads(encoded, object_pairs_hook=unique_object)
        if not isinstance(value, dict):
            raise ArtifactError("manifest must be an object")
        return value
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ArtifactError("missing or malformed manifest") from error


def require_hex(value, length: int, field: str) -> None:
    if (
        not isinstance(value, str)
        or re.fullmatch(f"[a-f0-9]{{{length}}}", value) is None
    ):
        raise ArtifactError(f"invalid {field}")


def validate_identity(identity: dict, brain_version: str, target: str) -> None:
    if not isinstance(identity, dict) or set(identity) != IDENTITY_FIELDS:
        raise ArtifactError("invalid identity fields")
    constants = {
        "schema_version": 1,
        "identity": "brain-frontend",
        "package": "@brain/frontend",
        "runtime_model": "bun-standalone",
        "bun_version": "1.4.4",
        "brain_version": brain_version,
        "target_triple": target,
    }
    for key, value in constants.items():
        if type(identity[key]) is not type(value) or identity[key] != value:
            raise ArtifactError(f"incompatible {key}")
    if target not in TARGETS:
        raise ArtifactError("unsupported target")
    for key in ["frontend_version", "brain_version"]:
        if (
            not isinstance(identity[key], str)
            or SEMVER.fullmatch(identity[key]) is None
        ):
            raise ArtifactError(f"invalid {key}")
    require_hex(identity["lockfile_sha256"], 64, "lockfile digest")
    source = identity["source"]
    if not isinstance(source, dict) or set(source) != SOURCE_FIELDS:
        raise ArtifactError("invalid source identity fields")
    require_hex(source["sha"], 40, "source SHA")
    require_hex(source["fingerprint_sha256"], 64, "source fingerprint")
    if (
        type(source["dirty"]) is not bool
        or source["fingerprint_algorithm"] != "brain-source-tree-v1"
        or type(source["untracked_input_count"]) is not int
        or source["untracked_input_count"] < 0
    ):
        raise ArtifactError("invalid source identity types")


def sanitized_environment() -> dict[str, str]:
    return {
        key: value
        for key, value in os.environ.items()
        if key
        not in {
            "BUN_BE_BUN",
            "BUN_OPTIONS",
            "NODE_OPTIONS",
            "BRAIN_FRONTEND_RUNTIME",
            "BRAIN_FRONTEND_VERSION",
            "BRAIN_FRONTEND_BUILD_ID",
        }
    }


def probe_identity(executable: Path, timeout: float = 5.0) -> dict:
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    process = subprocess.Popen(
        [str(executable), "--build-identity"],
        cwd=executable.parent,
        env=sanitized_environment(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        with selectors.DefaultSelector() as selector:
            for name, stream in [
                ("stdout", process.stdout),
                ("stderr", process.stderr),
            ]:
                os.set_blocking(stream.fileno(), False)
                selector.register(stream, selectors.EVENT_READ, name)
            deadline = time.monotonic() + timeout
            while selector.get_map():
                if time.monotonic() >= deadline:
                    raise ArtifactError(
                        "compiled identity diagnostic exceeded deadline"
                    )
                for key, _ in selector.select(
                    min(0.05, max(0, deadline - time.monotonic()))
                ):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    buffers[key.data].extend(chunk)
                    if sum(len(value) for value in buffers.values()) > 16 * 1024:
                        raise ArtifactError(
                            "compiled identity diagnostic exceeded output limit"
                        )
            remaining = max(0.001, deadline - time.monotonic())
            try:
                status = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired as error:
                raise ArtifactError(
                    "compiled identity diagnostic exceeded deadline"
                ) from error
        if status != 0 or buffers["stderr"]:
            raise ArtifactError("compiled identity diagnostic failed")
        try:
            identity = json.loads(buffers["stdout"], object_pairs_hook=unique_object)
        except (UnicodeError, json.JSONDecodeError) as error:
            raise ArtifactError("malformed compiled identity diagnostic") from error
        return identity
    finally:
        # Terminate the owned process group even if an executable fixture left
        # inherited pipes/children after its parent exited. No arbitrary PID use.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
        process.stdout.close()
        process.stderr.close()


def validate_frontend(root: Path, brain_version: str, target: str) -> dict:
    manifest = read_json(resolve_contained(root, "manifest.json"), 16 * 1024)
    if set(manifest) != IDENTITY_FIELDS | {"entrypoint", "executable_sha256"}:
        raise ArtifactError("invalid frontend manifest fields")
    identity = {key: manifest[key] for key in IDENTITY_FIELDS}
    validate_identity(identity, brain_version, target)
    if manifest["entrypoint"] != "brain-frontend":
        raise ArtifactError(
            "frontend entrypoint must be the fixed brain-frontend basename"
        )
    executable = resolve_contained(root, manifest["entrypoint"])
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise ArtifactError("frontend executable missing executable permission")
    require_hex(manifest["executable_sha256"], 64, "executable digest")
    if digest(executable) != manifest["executable_sha256"]:
        raise ArtifactError("frontend executable digest mismatch")
    compiled = probe_identity(executable)
    validate_identity(compiled, brain_version, target)
    if compiled != identity:
        raise ArtifactError("compiled identity does not match frontend manifest")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--brain-version", required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    try:
        validate_frontend(args.artifact, args.brain_version, args.target)
    except (ArtifactError, OSError) as error:
        raise SystemExit(f"error: {error}") from error
    print("PASS: frontend schema, containment, digest and compiled identity")


if __name__ == "__main__":
    main()
