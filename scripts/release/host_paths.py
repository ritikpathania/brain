"""Fail-closed scans of trusted built runtime files and native loader paths."""

import argparse
import os
import re
import subprocess
from pathlib import Path


class HostPathError(ValueError):
    """The delivery retains forbidden paths or unverifiable native linkage."""


FORBIDDEN = [
    re.compile(rb"/Users/"),
    re.compile(rb"/home/"),
    re.compile(rb"(?:^|[/\\])claude-(?:code|analysis)(?:[/\\]|\x00|$)"),
    re.compile(rb"(?:[/\\])uv(?:[/\\])(?:python|builds-v[0-9]+|cache)(?:[/\\])"),
]


def check_bytes(data: bytes, checkout: Path, forbidden_roots: list[str]) -> None:
    # Preserve existing daemon generic Windows-text distinction (C:/Users/ in comments)
    sanitized = data.replace(b"C:/Users/", b"")
    for pattern in FORBIDDEN:
        match = pattern.search(sanitized)
        if match:
            # Only report the forbidden marker, never a whole runtime string.
            marker = match.group().decode("ascii", "replace")
            raise HostPathError(f"forbidden runtime path marker: {marker}")
    for path in [str(checkout), *forbidden_roots]:
        if path and path.encode() in data:
            raise HostPathError("runtime retained an explicit forbidden build root")


def scan_host_paths(root: Path, checkout: Path, forbidden_roots: list[str]) -> None:
    if not root.is_dir():
        raise HostPathError("runtime scan root does not exist")
    try:
        for path in sorted(root.rglob("*")):
            if path.is_symlink():
                if not path.resolve(strict=True).is_relative_to(
                    root.resolve(strict=True)
                ):
                    raise HostPathError("runtime symlink escapes scan root")
                check_bytes(os.readlink(path).encode(), checkout, forbidden_roots)
            elif path.is_file():
                # Keep a bounded suffix to catch a forbidden path split across
                # reads; never swallow an unreadable file or scanner exception.
                suffix = b""
                overlap = max(
                    4096,
                    len(str(checkout).encode()),
                    *(len(item.encode()) for item in forbidden_roots),
                )
                with path.open("rb") as handle:
                    for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                        data = suffix + chunk
                        check_bytes(data, checkout, forbidden_roots)
                        suffix = data[-overlap:]
            elif not path.is_dir():
                raise HostPathError("unsupported runtime filesystem entry")
    except (OSError, RuntimeError) as error:
        raise HostPathError("cannot inspect runtime path") from error


def validate_loader_output(text: str, root: Path, platform: str) -> None:
    if "/Users/" in text or "/home/" in text or "not found" in text:
        raise HostPathError("loader retains a build-home or missing dependency")
    for line in text.splitlines():
        path = line.strip().split(" ", 1)[0]
        if not path:
            continue
        if path.startswith("/"):
            allowed = (
                ("/usr/lib/", "/System/Library/")
                if platform == "darwin"
                else (
                    "/lib/",
                    "/lib64/",
                    "/usr/lib/",
                    "/usr/lib64/",
                )
            )
            if not path.startswith(allowed):
                raise HostPathError("non-system absolute native loader path")
        elif path.startswith(("@loader_path/", "@executable_path/", "$ORIGIN/")):
            continue
        elif path.startswith("@rpath/"):
            continue
        elif platform == "darwin":
            raise HostPathError("unrecognized Mach-O loader path")


def inspect_native(executable: Path, target: str) -> None:
    env = {
        key: value
        for key, value in os.environ.items()
        if key
        not in {
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "DYLD_LIBRARY_PATH",
            "DYLD_FALLBACK_LIBRARY_PATH",
            "DYLD_FALLBACK_FRAMEWORK_PATH",
            "DYLD_INSERT_LIBRARIES",
        }
    }

    def run(arguments: list[str]) -> str:
        result = subprocess.run(
            arguments, env=env, capture_output=True, text=True, check=False, timeout=10
        )
        if result.returncode:
            raise HostPathError("native loader inspection command failed")
        return result.stdout

    if target == "aarch64-apple-darwin":
        header = run(["file", str(executable)])
        if "Mach-O 64-bit executable arm64" not in header:
            raise HostPathError("frontend architecture disagrees with target")
        libraries = [
            line.strip().split(" ", 1)[0]
            for line in run(["otool", "-L", str(executable)]).splitlines()[1:]
        ]
        load_commands = run(["otool", "-l", str(executable)])
        rpaths = re.findall(r"cmd LC_RPATH\n.*?\n\s*path (\S+)", load_commands)
        validate_loader_output(
            "\n".join(libraries + rpaths), executable.parent, "darwin"
        )
        for library in libraries:
            if not library.startswith(("/usr/lib/", "/System/Library/")):
                # Frontend is standalone: no application/native package dylib.
                raise HostPathError(
                    "standalone frontend has an external native dependency"
                )
    elif target == "x86_64-unknown-linux-gnu":
        header = run(["readelf", "-h", str(executable)])
        if "ELF64" not in header or "X86-64" not in header:
            raise HostPathError("frontend architecture disagrees with target")
        dynamic = run(["readelf", "-d", str(executable)])
        for paths in re.findall(r"(?:RPATH|RUNPATH).*?\[(.*?)\]", dynamic):
            validate_loader_output(
                "\n".join(paths.split(":")), executable.parent, "linux"
            )
        linked = run(["ldd", str(executable)])
        validate_loader_output(linked, executable.parent, "linux")
        for dependency in re.findall(r"=>\s+(\S+)", linked):
            validate_loader_output(dependency, executable.parent, "linux")
    else:
        raise HostPathError("unsupported native target")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--checkout", type=Path, required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    try:
        scan_host_paths(args.artifact, args.checkout, [])
        frontend_bin = args.artifact / "frontend" / "brain-frontend" / "brain-frontend"
        if not frontend_bin.is_file():
            frontend_bin = args.artifact / "brain-frontend"
        inspect_native(frontend_bin, args.target)
    except (HostPathError, OSError, subprocess.TimeoutExpired) as error:
        raise SystemExit(f"error: {error}") from error
    print("PASS: runtime host-path and native loader scans")


if __name__ == "__main__":
    main()
