#!/usr/bin/env bash
# Verify a daemon-only runtime artifact without using its source checkout or a
# developer-home Python installation at runtime.

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/verify-daemon-runtime.sh --artifact <directory> [--repeat <count>]

The verifier checks the portable runtime manifest, rejects obvious build-host
paths from runtime binaries, archives then extracts the artifact into a fresh
temporary directory, starts the extracted daemon with an empty Python
environment, and requires the daemon's existing Python initialization phase to
reach healthy status.
EOF
}

artifact=""
repeat=1

while [ "$#" -gt 0 ]; do
    case "$1" in
        --artifact)
            artifact="${2:-}"
            shift 2
            ;;
        --repeat)
            repeat="${2:-}"
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

if [ -z "$artifact" ] || [ ! -d "$artifact" ]; then
    echo "error: --artifact must name an artifact directory" >&2
    exit 2
fi
case "$repeat" in
    ''|*[!0-9]*)
        echo "error: --repeat must be a positive integer" >&2
        exit 2
        ;;
esac
if [ "$repeat" -lt 1 ]; then
    echo "error: --repeat must be at least 1" >&2
    exit 2
fi

daemon="$artifact/bin/brain-daemon"
runtime="$artifact/runtime/python"
manifest="$artifact/runtime-manifest.json"
file_manifest="$artifact/runtime/python-files.json"
for required in "$daemon" "$runtime" "$manifest" "$file_manifest"; do
    if [ ! -e "$required" ]; then
        echo "error: incomplete daemon runtime artifact; missing $required" >&2
        exit 1
    fi
done
if [ ! -x "$daemon" ]; then
    echo "error: packaged daemon is not executable: $daemon" >&2
    exit 1
fi
command -v python3 >/dev/null || {
    echo "error: python3 is required for artifact manifest verification" >&2
    exit 2
}
command -v cc >/dev/null || {
    echo "error: cc is required for the compiled-extension extraction smoke" >&2
    exit 2
}

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
resolved_initial_lib="$(python3 "$repository_root/scripts/release/runtime_manifest.py" --artifact "$artifact")"

ARTIFACT_ROOT="$artifact" python3 - <<'PY'
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


root = Path(os.environ["ARTIFACT_ROOT"])
manifest = json.loads((root / "runtime-manifest.json").read_text())
if manifest.get("schema_version") != 1 or manifest.get("artifact") != "brain-daemon-runtime":
    raise SystemExit("invalid daemon runtime manifest")
if not str(manifest.get("python", {}).get("version", "")).startswith("3.15."):
    raise SystemExit("artifact does not declare CPython 3.15")
if manifest.get("python", {}).get("home") != "runtime/python":
    raise SystemExit("artifact does not use the artifact-relative Python home")
if digest(root / "bin/brain-daemon") != manifest.get("daemon_sha256"):
    raise SystemExit("daemon binary digest does not match runtime manifest")
python_home = root / manifest["python"]["home"]
if digest(python_home / manifest["python"]["library"]) != manifest["python"].get("library_sha256"):
    raise SystemExit("CPython library digest does not match runtime manifest")

files = json.loads((root / manifest["runtime_files_manifest"]).read_text()).get("files", [])
for entry in files:
    path = root / "runtime/python" / entry["path"]
    if entry["type"] == "file":
        if not path.is_file() or digest(path) != entry["sha256"]:
            raise SystemExit(f"runtime file digest mismatch: {entry['path']}")
    elif entry["type"] == "symlink":
        if not path.is_symlink() or os.readlink(path) != entry["target"]:
            raise SystemExit(f"runtime symlink mismatch: {entry['path']}")
    else:
        raise SystemExit(f"unknown runtime manifest entry type: {entry['type']}")
PY

python_library_info="$(ARTIFACT_ROOT="$artifact" python3 - <<'PY'
import json
import os
from pathlib import Path

root = Path(os.environ["ARTIFACT_ROOT"])
manifest = json.loads((root / "runtime-manifest.json").read_text())
py_section = manifest.get("python", {})
print(py_section.get("home", "runtime/python"))
print(py_section.get("library", ""))
print(py_section.get("version", ""))
PY
)"
python_home_rel="$(printf '%s\n' "$python_library_info" | sed -n '1p')"
python_library_rel="$(printf '%s\n' "$python_library_info" | sed -n '2p')"
python_version="$(printf '%s\n' "$python_library_info" | sed -n '3p')"
python_library_name="$(basename "$python_library_rel")"

if find "$artifact/bin" "$artifact/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F "$repository_root" >/dev/null; then
    echo "error: artifact retained the source checkout path" >&2
    exit 1
fi
if find "$artifact/bin" "$artifact/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F '/Users/' | grep -Fv 'C:/Users/' >/dev/null; then
    echo "error: artifact retained a macOS developer-home path" >&2
    exit 1
fi
if find "$artifact/bin" "$artifact/runtime/python" -type f -print0 |
    xargs -0 strings 2>/dev/null | grep -F '/home/' >/dev/null; then
    echo "error: artifact retained a Linux developer-home path" >&2
    exit 1
fi

case "$(uname -s)" in
    Darwin)
        if otool -L "$daemon" | grep -Eq '/Users/|/home/|'"$repository_root"; then
            echo "error: daemon Mach-O load commands retain a build-host path" >&2
            exit 1
        fi
        if ! otool -L "$daemon" | grep -Fq "@rpath/$python_library_name"; then
            echo "error: daemon does not load libpython through the packaged rpath (@rpath/$python_library_name)" >&2
            exit 1
        fi
        if ! otool -l "$daemon" | grep -Fq '@loader_path/../runtime/python/lib'; then
            echo "error: daemon is missing the artifact-relative Python rpath" >&2
            exit 1
        fi
        if otool -l "$daemon" | awk '
            $2 == "LC_RPATH" { in_rpath = 1; next }
            in_rpath && $1 == "path" {
                if ($2 != "@loader_path/../runtime/python/lib") exit 1
                in_rpath = 0
            }
        '; then
            :
        else
            echo "error: daemon retains a non-artifact-relative Mach-O rpath" >&2
            exit 1
        fi
        ;;
    Linux)
        command -v readelf >/dev/null || {
            echo "error: readelf is required to inspect Linux daemon linkage" >&2
            exit 2
        }
        if readelf -d "$daemon" | grep -Eq '/home/|/Users/|'"$repository_root"; then
            echo "error: daemon ELF dynamic section retains a build-host path" >&2
            exit 1
        fi
        if ! readelf -d "$daemon" | grep -Fq '$ORIGIN/../runtime/python/lib'; then
            echo "error: daemon is missing the artifact-relative Python rpath" >&2
            exit 1
        fi
        ;;
    *)
        echo "error: unsupported runtime verification platform: $(uname -s)" >&2
        exit 2
        ;;
esac

for iteration in $(seq 1 "$repeat"); do
    # UDS paths on macOS have a small fixed maximum. Keep the clean-extraction
    # root short rather than inheriting a potentially long per-user TMPDIR.
    temporary_root="$(mktemp -d /tmp/brainrt.XXXXXX)"
    archive="$temporary_root/daemon-runtime.tar.gz"
    extracted="$temporary_root/extracted"
    runtime_root="$temporary_root/r"
    daemon_pid=""

    cleanup() {
        if [ -n "$daemon_pid" ] && kill -0 "$daemon_pid" 2>/dev/null; then
            kill -TERM "$daemon_pid" 2>/dev/null || true
            wait "$daemon_pid" 2>/dev/null || true
        fi
        rm -rf "$temporary_root"
    }
    trap cleanup EXIT INT TERM

    tar -czf "$archive" -C "$artifact" .
    mkdir -p "$extracted"
    tar -xzf "$archive" -C "$extracted"
    mkdir -p "$runtime_root/h" "$runtime_root/c" "$runtime_root/l"

    # The artifact intentionally ships an embeddable CPython runtime rather
    # than a second `bin/python`. Compile a tiny host-side probe which dlopens
    # the extracted libpython, initializes it with the extracted PYTHONHOME,
    # and imports `_crypt`. This proves a compiled lib-dynload extension is
    # actually loadable from the extracted artifact rather than merely present
    # in its manifest.
    extension_probe="$runtime_root/compiled-extension-probe"
    cc_command=(cc -x c - -o "$extension_probe")
    if [ "$(uname -s)" = "Linux" ]; then
        cc_command+=(-ldl)
    fi
    "${cc_command[@]}" <<'C'
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>

typedef void (*PyInitialize)(void);
typedef int (*PyRunSimpleString)(const char *);
typedef int (*PyFinalizeEx)(void);

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: compiled-extension-probe <python-home> <libpython>\n");
        return 2;
    }

    if (setenv("PYTHONHOME", argv[1], 1) != 0 ||
        setenv("BRAIN_RUNTIME_PYTHON_HOME", argv[1], 1) != 0) {
        perror("setenv");
        return 1;
    }

    void *library = dlopen(argv[2], RTLD_NOW | RTLD_GLOBAL);
    if (library == NULL) {
        fprintf(stderr, "failed to load packaged libpython: %s\n", dlerror());
        return 1;
    }

    PyInitialize initialize = (PyInitialize)dlsym(library, "Py_Initialize");
    PyRunSimpleString run_simple_string =
        (PyRunSimpleString)dlsym(library, "PyRun_SimpleString");
    PyFinalizeEx finalize = (PyFinalizeEx)dlsym(library, "Py_FinalizeEx");
    if (initialize == NULL || run_simple_string == NULL || finalize == NULL) {
        fprintf(stderr, "packaged libpython is missing the required embedding symbols\n");
        dlclose(library);
        return 1;
    }

    initialize();
    int import_result = run_simple_string(
        "try:\n"
        "    import _dbm as ext\n"
        "except ImportError:\n"
        "    import _crypt as ext\n"
        "import os\n"
        "from pathlib import Path\n"
        "runtime = Path(os.environ['BRAIN_RUNTIME_PYTHON_HOME']).resolve()\n"
        "module = Path(ext.__file__).resolve()\n"
        "assert runtime in module.parents, module\n"
        "assert 'lib-dynload' in module.parts, module\n"
        "print(f'compiled-extension={module}')\n"
    );
    int finalize_result = finalize();
    dlclose(library);
    return import_result == 0 && finalize_result == 0 ? 0 : 1;
}
C
    extracted_lib="$(python3 "$repository_root/scripts/release/runtime_manifest.py" --artifact "$extracted")"
    env -i \
        PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
        "$extension_probe" \
        "$extracted/$python_home_rel" \
        "$extracted_lib"
    echo "Clean-extraction compiled-extension smoke passed ($iteration/$repeat)"

    health_port="$(python3 - <<'PY'
import socket
with socket.socket() as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
)"
    # PYTHONHOME/PYTHONPATH are deliberately absent: successful startup proves
    # the extracted binary uses its own runtime/python directory before PyO3
    # initializes CPython. The daemon's startup path includes the mandatory
    # Python invariant phase, so /status cannot become healthy without it.
    env -i \
        PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
        HOME="$runtime_root/h" \
        BRAIN_CONFIG_DIR="$runtime_root/c" \
        BRAIN_SOCKET_PATH="$runtime_root/c/daemon.sock" \
        BRAIN_PID_PATH="$runtime_root/c/daemon.pid" \
        BRAIN_LOG_PATH="$runtime_root/l/daemon.log" \
        BRAIN_DB_PATH="$runtime_root/c/brain_runtime.db" \
        BRAIN_HEALTH_PORT="$health_port" \
        "$extracted/bin/brain-daemon" daemon run >"$runtime_root/daemon.log" 2>&1 &
    daemon_pid="$!"

    ready=""
    for _ in $(seq 1 100); do
        if curl --fail --silent --show-error "http://127.0.0.1:$health_port/status" \
            >"$runtime_root/status.json" 2>/dev/null; then
            if grep -Fq '"health":"healthy"' "$runtime_root/status.json"; then
                ready=1
                break
            fi
        fi
        if ! kill -0 "$daemon_pid" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done

    if [ -z "$ready" ]; then
        echo "error: extracted daemon did not reach healthy status on iteration $iteration" >&2
        cat "$runtime_root/daemon.log" >&2 || true
        exit 1
    fi

    kill -TERM "$daemon_pid"
    wait "$daemon_pid"
    daemon_pid=""
    echo "Clean-extraction Python startup smoke passed ($iteration/$repeat)"
    trap - EXIT INT TERM
    cleanup
done
