"""Product extraction smoke test runner using standard library PTY and selectors."""

import argparse
import fcntl
import os
import pty
import re
import selectors
import signal
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path


class ProductSmokeError(RuntimeError):
    """Product smoke execution failed."""


def owned_pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
        return True
    except OSError, ProcessLookupError:
        return False


def reap_process(
    proc: subprocess.Popen,
    extra_pids: list[int] | None = None,
    timeout: float = 3.0,
):
    pids = [proc.pid]
    if extra_pids:
        pids.extend(extra_pids)
    for pid in pids:
        if owned_pid_alive(pid):
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not any(owned_pid_alive(pid) for pid in pids):
            break
        time.sleep(0.05)
    for pid in pids:
        if owned_pid_alive(pid):
            try:
                os.kill(pid, signal.SIGKILL)
            except OSError:
                pass
    try:
        proc.wait(timeout=1.0)
    except Exception:
        pass


ANSI_REGEX = re.compile(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])")


def strip_ansi(text: str) -> str:
    return ANSI_REGEX.sub("", text)


def run_product_smoke(
    extracted_root: Path,
    runtime_root: Path,
    unrelated_cwd: Path,
    test_fault_step: str | None = None,
) -> dict:
    canonical_extracted = extracted_root.resolve(strict=True)
    cli_bin = canonical_extracted / "bin" / "brain"
    if not cli_bin.is_file() or not os.access(cli_bin, os.X_OK):
        raise ProductSmokeError(f"CLI binary missing or not executable: {cli_bin}")

    # Set up isolated directories
    home_dir = runtime_root / "h"
    config_dir = runtime_root / "c"
    state_dir = runtime_root / "s"
    log_dir = runtime_root / "l"
    data_dir = runtime_root / "d"
    for d in [home_dir, config_dir, state_dir, log_dir, data_dir, unrelated_cwd]:
        d.mkdir(parents=True, exist_ok=True)

    socket_path = config_dir / "daemon.sock"
    pid_path = config_dir / "daemon.pid"

    env = {
        "HOME": str(home_dir),
        "XDG_CONFIG_HOME": str(config_dir),
        "XDG_STATE_HOME": str(state_dir),
        "BRAIN_CONFIG_DIR": str(config_dir),
        "BRAIN_SOCKET_PATH": str(socket_path),
        "BRAIN_PID_PATH": str(pid_path),
        "BRAIN_LOG_PATH": str(log_dir / "daemon.log"),
        "BRAIN_DB_PATH": str(data_dir / "brain.db"),
        "BRAIN_ANALYTICS_DB_PATH": str(data_dir / "analytics.db"),
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "TERM": "xterm-256color",
        "CI": "false",
        "CONTINUOUS_INTEGRATION": "false",
        "FORCE_COLOR": "0",
    }

    # Open PTY
    master, slave = pty.openpty()
    try:
        winsize = struct.pack("HHHH", 28, 90, 0, 0)
        fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)
    except Exception:
        pass

    proc = None
    extra_pids = []
    output_chunks = []
    sel = selectors.DefaultSelector()

    try:
        proc = subprocess.Popen(
            [str(cli_bin), "ui"],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            cwd=str(unrelated_cwd),
            env=env,
            start_new_session=True,
        )
        os.close(slave)
        slave = -1

        sel.register(master, selectors.EVENT_READ)

        def wait_for_text(
            expected: str, since_len: int = 0, timeout_sec: float = 15.0
        ) -> str:
            deadline = time.monotonic() + timeout_sec
            while time.monotonic() < deadline:
                events = sel.select(timeout=0.1)
                for key, _ in events:
                    try:
                        data = os.read(key.fd, 4096)
                        if data:
                            output_chunks.append(data.decode("utf-8", errors="replace"))
                    except OSError:
                        pass
                current = strip_ansi("".join(output_chunks))
                search_slice = current[since_len:] if since_len < len(current) else ""
                if expected in search_slice:
                    return current
                if proc.poll() is not None:
                    code = proc.returncode
                    recent = current[-1000:]
                    raise ProductSmokeError(
                        f"process exited prematurely with code {code} "
                        f"waiting for '{expected}'; output={recent}"
                    )
            recent_out = strip_ansi("".join(output_chunks))[-1000:]
            raise ProductSmokeError(
                f"timed out waiting for '{expected}'; output={recent_out}"
            )

        # Step 1: Wait for session picker readiness
        wait_for_text("New conversation")

        # Step 2: Select new conversation
        os.write(master, b"\r")
        wait_for_text("Connected · New session")

        # Step 3: Check fault injection point
        if test_fault_step == "after_submission":
            raise ProductSmokeError("Injected fault after submission")

        # Step 4: Type smoke prompt and wait for echo
        os.write(master, b"smoke test")
        wait_for_text("smoke test")

        # Step 5: Submit prompt with Enter and wait for response
        submitted_at = len(strip_ansi("".join(output_chunks)))
        os.write(master, b"\r")
        wait_for_text("Mock response to:", since_len=submitted_at, timeout_sec=20.0)
        wait_for_text("Message Brain", since_len=submitted_at, timeout_sec=10.0)

        # Step 6: Clean exit with Ctrl+C when composer is idle
        os.write(master, b"\x03")

        # Wait for exit
        exit_deadline = time.monotonic() + 5.0
        while time.monotonic() < exit_deadline:
            if proc.poll() is not None:
                break
            time.sleep(0.05)

        if proc.poll() is None:
            try:
                proc.terminate()
                proc.wait(timeout=2.0)
            except Exception:
                try:
                    proc.kill()
                    proc.wait(timeout=1.0)
                except Exception:
                    pass

        # Stop daemon cleanly using extracted binary
        daemon_bin = canonical_extracted / "bin" / "brain-daemon"
        if pid_path.is_file():
            try:
                d_pid = int(pid_path.read_text().strip())
                extra_pids.append(d_pid)
            except ValueError:
                pass
        if daemon_bin.is_file() and os.access(daemon_bin, os.X_OK):
            subprocess.run(
                [str(daemon_bin), "daemon", "stop"],
                env=env,
                capture_output=True,
                timeout=5,
            )

    except Exception as e:
        captured = strip_ansi("".join(output_chunks))
        sys.stderr.write(
            f"Product smoke exception: {e}\nCaptured PTY output:\n{captured}\n"
        )
        raise
    finally:
        if slave >= 0:
            try:
                os.close(slave)
            except OSError:
                pass
        try:
            sel.unregister(master)
        except Exception:
            pass
        try:
            os.close(master)
        except OSError:
            pass

        if proc is not None:
            reap_process(proc, extra_pids)

    return {
        "status": "passed",
        "output": strip_ansi("".join(output_chunks)),
    }


def main():
    parser = argparse.ArgumentParser(description="Product extraction smoke runner")
    parser.add_argument(
        "--extracted", required=True, type=Path, help="Extracted product root"
    )
    parser.add_argument(
        "--runtime-root", required=True, type=Path, help="Temporary runtime root"
    )
    parser.add_argument(
        "--cwd", required=True, type=Path, help="Unrelated working directory"
    )
    args = parser.parse_args()

    try:
        run_product_smoke(args.extracted, args.runtime_root, args.cwd)
        print("Product smoke passed")
    except Exception as e:
        sys.stderr.write(f"error: {e}\n")
        sys.exit(1)


if __name__ == "__main__":
    main()
