#!/usr/bin/env bash
# Build a daemon-only, relocatable CPython runtime layout.
#
# The resulting directory deliberately contains no frontend. Its stable layout is:
#   <artifact>/bin/brain-daemon
#   <artifact>/runtime/python
#   <artifact>/runtime-manifest.json
#   <artifact>/runtime/python-files.json
#
# The Python runtime is the CPython base prefix selected for the PyO3 build, not
# the daemon development virtual environment. Third-party Python plugins remain
# independently installed plugin inputs; this artifact supplies the interpreter
# and standard library required to initialize the embedded bridge.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/package-daemon-runtime.sh --daemon <path> --output <new-directory> --target <target-triple>

Requires PYO3_PYTHON to name the CPython 3.15 build interpreter used to build the
daemon. The output path must not already exist.
EOF
}

daemon_binary=""
output_directory=""
target_triple=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --daemon)
            daemon_binary="${2:-}"
            shift 2
            ;;
        --output)
            output_directory="${2:-}"
            shift 2
            ;;
        --target)
            target_triple="${2:-}"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [ -z "$daemon_binary" ] || [ -z "$output_directory" ] || [ -z "$target_triple" ]; then
    echo "error: --daemon, --output, and --target are required" >&2
    usage >&2
    exit 2
fi
if [ ! -f "$daemon_binary" ] || [ ! -x "$daemon_binary" ]; then
    echo "error: daemon binary is missing or not executable: $daemon_binary" >&2
    exit 2
fi
if [ -e "$output_directory" ]; then
    echo "error: output directory must not already exist: $output_directory" >&2
    exit 2
fi

pyo3_python="${PYO3_PYTHON:-}"
if [ -z "$pyo3_python" ] || [ ! -x "$pyo3_python" ]; then
    echo "error: PYO3_PYTHON must name the executable CPython 3.15 build interpreter" >&2
    exit 2
fi

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
python_version_pin="$(tr -d '[:space:]' < "$repository_root/daemon/.python-version")"
if [ -z "$python_version_pin" ]; then
    echo "error: daemon/.python-version must pin the bundled CPython runtime" >&2
    exit 2
fi

python_details="$("$pyo3_python" - <<'PY'
import pathlib
import sys
import sysconfig

if sys.version_info[:2] != (3, 15):
    raise SystemExit(f"expected CPython 3.15, found {sys.version}")

base = pathlib.Path(sys.base_prefix).resolve()
libdir = pathlib.Path(sysconfig.get_config_var("LIBDIR") or base / "lib").resolve()
library = libdir / (sysconfig.get_config_var("LDLIBRARY") or "")
if not library.is_file():
    raise SystemExit(f"CPython shared library is missing: {library}")
print(base)
print(library)
print(".".join(map(str, sys.version_info[:3])))
PY
)"
python_home="$(printf '%s\n' "$python_details" | sed -n '1p')"
source_python_library="$(printf '%s\n' "$python_details" | sed -n '2p')"
python_version="$(printf '%s\n' "$python_details" | sed -n '3p')"
library_name="$(basename "$source_python_library")"
base_interpreter="$python_home/bin/python3"
if [ ! -x "$base_interpreter" ]; then
    base_interpreter="$pyo3_python"
fi
if [ "$python_version" != "$python_version_pin" ]; then
    echo "error: PYO3_PYTHON is $python_version but daemon/.python-version requires $python_version_pin" >&2
    exit 2
fi

case "$source_python_library" in
    "$python_home"/*) ;;
    *)
        echo "error: CPython shared library is outside its base prefix: $source_python_library" >&2
        exit 2
        ;;
esac

output_parent="$(dirname "$output_directory")"
if [ ! -d "$output_parent" ]; then
    echo "error: output parent directory does not exist: $output_parent" >&2
    exit 2
fi

mkdir -p "$output_directory/bin" "$output_directory/runtime/python"
cp "$daemon_binary" "$output_directory/bin/brain-daemon"

case "$(uname -s)" in
    Darwin)
        # The embedded interpreter needs the dynamic library and standard
        # library, not CPython's build/developer executables or headers.
        # ditto preserves the remaining internal symlinks without
        # dereferencing them into paths outside the artifact.
        ditto "$python_home/lib" "$output_directory/runtime/python/lib"
        ;;
    Linux)
        cp -a "$python_home/lib" "$output_directory/runtime/python/lib"
        ;;
    *)
        echo "error: unsupported runtime packaging platform: $(uname -s)" >&2
        exit 2
        ;;
esac

# Bytecode cache files record the CPython build machine's source paths and are
# not required by an embedded interpreter. Shipping source modules lets CPython
# compile cache files into an operator-writable location if desired, while the
# artifact itself remains free of developer-home paths.
find "$output_directory/runtime/python" -type f -name '*.pyc' -delete
find "$output_directory/runtime/python" -type d -name '__pycache__' -empty -delete

# CPython's generated sysconfig module records the interpreter's installation
# prefix. Rewrite that build-time prefix to a marker resolved from sys.prefix at
# import time, so plugin code can still use sysconfig after relocation without
# retaining the build host in the distributed runtime.
PACKAGED_PYTHON_HOME="$output_directory/runtime/python" \
BUILD_PYTHON_HOME="$python_home" \
"$pyo3_python" - <<'PY'
import os
from pathlib import Path

runtime_home = Path(os.environ["PACKAGED_PYTHON_HOME"])
build_home = os.environ["BUILD_PYTHON_HOME"]
modules = sorted((runtime_home / "lib").glob("python3.*/_sysconfigdata_*.py"))
if not modules:
    raise SystemExit("bundled CPython has no sysconfig data module")

for module in modules:
    source = module.read_text()
    if build_home not in source:
        raise SystemExit(f"sysconfig data module has no expected build prefix: {module}")
    rewritten = source.replace(build_home, "__BRAIN_PYTHON_HOME__")
    rewritten = (
        "import sys as _brain_runtime_sys\n"
        + rewritten
        + "\nfor _brain_key, _brain_value in tuple(build_time_vars.items()):\n"
        + "    if isinstance(_brain_value, str):\n"
        + "        build_time_vars[_brain_key] = _brain_value.replace(\n"
        + "            \"__BRAIN_PYTHON_HOME__\", _brain_runtime_sys.prefix\n"
        + "        )\n"
    )
    module.write_text(rewritten)

json_modules = sorted((runtime_home / "lib").glob("python3.*/_sysconfig_vars__*.json"))
for json_module in json_modules:
    source = json_module.read_text()
    rewritten = (
        source.replace(build_home, "__BRAIN_PYTHON_HOME__")
        .replace("/Users/runner", "__BRAIN_USER_BASE__")
        .replace("/home/runner", "__BRAIN_USER_BASE__")
    )
    json_module.write_text(rewritten)
PY

PYTHONHOME="$output_directory/runtime/python" \
"$base_interpreter" -B -S -c '
import os
import sys
import sysconfig
assert sys.prefix == os.environ["PYTHONHOME"]
assert sysconfig.get_config_var("prefix") == sys.prefix
'

packaged_python_library="$output_directory/runtime/python/lib/$library_name"
if [ ! -f "$packaged_python_library" ]; then
    echo "error: bundled CPython library is missing: $packaged_python_library" >&2
    exit 2
fi

case "$(uname -s)" in
    Darwin)
        command -v install_name_tool >/dev/null || {
            echo "error: install_name_tool is required to package a macOS daemon runtime" >&2
            exit 2
        }
        command -v codesign >/dev/null || {
            echo "error: codesign is required to ad-hoc sign rewritten Mach-O files" >&2
            exit 2
        }
        command -v otool >/dev/null || {
            echo "error: otool is required to verify macOS daemon linkage" >&2
            exit 2
        }

        install_name_tool -id "@rpath/$library_name" "$packaged_python_library"
        install_name_tool -change "$source_python_library" "@rpath/$library_name" \
            "$output_directory/bin/brain-daemon"
        # The Rust build uses framework search paths on developer machines, but
        # no packaged daemon dependency needs those absolute directories after
        # libpython is rewritten above. Keep the artifact's rpath surface
        # limited to its own CPython runtime.
        while IFS= read -r existing_rpath; do
            case "$existing_rpath" in
                /*)
                    install_name_tool -delete_rpath "$existing_rpath" \
                        "$output_directory/bin/brain-daemon"
                    ;;
            esac
        done < <(otool -l "$output_directory/bin/brain-daemon" | awk '
            $2 == "LC_RPATH" { in_rpath = 1; next }
            in_rpath && $1 == "path" { print $2; in_rpath = 0 }
        ')
        if ! otool -l "$output_directory/bin/brain-daemon" | grep -Fq '@loader_path/../runtime/python/lib'; then
            install_name_tool -add_rpath '@loader_path/../runtime/python/lib' \
                "$output_directory/bin/brain-daemon"
        fi
        codesign --force --sign - "$packaged_python_library" >/dev/null
        codesign --force --sign - "$output_directory/bin/brain-daemon" >/dev/null
        ;;
    Linux)
        command -v patchelf >/dev/null || {
            echo "error: patchelf is required to package a Linux daemon runtime" >&2
            exit 2
        }
        command -v readelf >/dev/null || {
            echo "error: readelf is required to inspect Linux daemon linkage" >&2
            exit 2
        }
        linked_python_library="$(readelf -d "$output_directory/bin/brain-daemon" | sed -n 's/.*Shared library: \[\(libpython3\.15[^]]*\)\].*/\1/p' | head -n 1)"
        if [ -z "$linked_python_library" ]; then
            echo "error: daemon does not dynamically link a CPython 3.15 library" >&2
            exit 2
        fi
        patchelf --replace-needed "$linked_python_library" "$library_name" \
            "$output_directory/bin/brain-daemon"
        patchelf --set-rpath '$ORIGIN/../runtime/python/lib' \
            "$output_directory/bin/brain-daemon"
        ;;
esac

if find "$output_directory/bin" "$output_directory/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F "$repository_root" >/dev/null; then
    echo "error: packaged daemon runtime retained the source checkout path" >&2
    exit 1
fi
if find "$output_directory/bin" "$output_directory/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F '/Users/' | grep -Fv 'C:/Users/' >/dev/null; then
    echo "error: packaged daemon runtime retained a macOS developer-home path" >&2
    exit 1
fi
if find "$output_directory/bin" "$output_directory/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F '/home/' >/dev/null; then
    echo "error: packaged daemon runtime retained a Linux developer-home path" >&2
    exit 1
fi

daemon_source_sha="$(git rev-parse --verify HEAD)"
source_dirty=false
if ! git diff --quiet HEAD -- 2>/dev/null || [ -n "$(git ls-files --others --exclude-standard 2>/dev/null)" ]; then
    source_dirty=true
fi
untracked_file_count="$(git ls-files --others --exclude-standard 2>/dev/null | wc -l | tr -d '[:space:]')"
rust_version="$(rustc --version)"
# Running an embedded CPython binary while assembling the artifact can generate
# platform sysconfig data containing the build prefix. Cargo package metadata is
# the authoritative version source and avoids mutating the bundled runtime.
daemon_version="$(awk -F '"' '
    /^\[workspace.package\]$/ { in_workspace_package = 1; next }
    in_workspace_package && /^version = / { print $2; exit }
' "$repository_root/Cargo.toml")"
if [ -z "$daemon_version" ]; then
    daemon_version="$(awk -F '"' '
        /^\[package\]$/ { in_package = 1; next }
        in_package && /^version = / { print $2; exit }
    ' "$repository_root/daemon/Cargo.toml")"
fi
if [ -z "$daemon_version" ]; then
    echo "error: unable to resolve brain-daemon package version" >&2
    exit 2
fi

PACKAGED_RUNTIME="$output_directory/runtime/python" \
PACKAGED_DAEMON="$output_directory/bin/brain-daemon" \
PACKAGED_MANIFEST="$output_directory/runtime-manifest.json" \
PACKAGED_FILE_MANIFEST="$output_directory/runtime/python-files.json" \
PACKAGED_TARGET="$target_triple" \
PACKAGED_DAEMON_SOURCE_SHA="$daemon_source_sha" \
PACKAGED_SOURCE_DIRTY="$source_dirty" \
PACKAGED_UNTRACKED_FILE_COUNT="$untracked_file_count" \
PACKAGED_DAEMON_VERSION="$daemon_version" \
PACKAGED_PYTHON_VERSION="$python_version" \
PACKAGED_PYTHON_LIBRARY="lib/$library_name" \
PACKAGED_RUST_VERSION="$rust_version" \
python3 - <<'PY'
import hashlib
import json
import os
from pathlib import Path


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


runtime = Path(os.environ["PACKAGED_RUNTIME"])
entries = []
for path in sorted(runtime.rglob("*")):
    relative = path.relative_to(runtime).as_posix()
    if path.is_symlink():
        entries.append({"path": relative, "type": "symlink", "target": os.readlink(path)})
    elif path.is_file():
        entries.append({"path": relative, "type": "file", "sha256": digest(path)})

file_manifest = Path(os.environ["PACKAGED_FILE_MANIFEST"])
file_manifest.write_text(json.dumps({"schema_version": 1, "files": entries}, indent=2) + "\n")

daemon = Path(os.environ["PACKAGED_DAEMON"])
library = runtime / os.environ["PACKAGED_PYTHON_LIBRARY"]
manifest = {
    "schema_version": 1,
    "artifact": "brain-daemon-runtime",
    "brain_version": os.environ["PACKAGED_DAEMON_VERSION"],
    "daemon_source_sha": os.environ["PACKAGED_DAEMON_SOURCE_SHA"],
    "source_tree": {
        "dirty": os.environ["PACKAGED_SOURCE_DIRTY"] == "true",
        "untracked_file_count": int(os.environ["PACKAGED_UNTRACKED_FILE_COUNT"]),
    },
    "target_triple": os.environ["PACKAGED_TARGET"],
    "rust_version": os.environ["PACKAGED_RUST_VERSION"],
    "python": {
        "version": os.environ["PACKAGED_PYTHON_VERSION"],
        "home": "runtime/python",
        "library": os.environ["PACKAGED_PYTHON_LIBRARY"],
        "library_sha256": digest(library),
    },
    "daemon_sha256": digest(daemon),
    "runtime_files_manifest": "runtime/python-files.json",
    "contents": [
        "bin/brain-daemon",
        "runtime/python",
        "runtime-manifest.json",
        "runtime/python-files.json",
    ],
}
Path(os.environ["PACKAGED_MANIFEST"]).write_text(json.dumps(manifest, indent=2) + "\n")
PY

echo "Packaged relocatable CPython $python_version daemon runtime at $output_directory"
