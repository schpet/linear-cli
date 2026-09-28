#!/usr/bin/env python3
"""Bounded synthetic PTY witness for the C024F1 selector example."""
import argparse
import fcntl
import os
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

LIMIT_SECONDS = 10
LIMIT_BYTES = 1 << 20


def probe(binary: str, name: str, keys: bytes, ci: str | None, expected: int, marker: bytes | None,
          burst: bool = False, rows: int = 40, columns: int = 80,
          fail_write: bool = False) -> None:
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
    terminal_before = termios.tcgetattr(slave)
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm", "NO_COLOR": "1"}
    if ci is not None:
        env["CI"] = ci
    if fail_write:
        env["C024F1_FAIL_WRITE"] = "1"
    with tempfile.TemporaryDirectory(prefix="c024f1-pty-") as cwd:
        child = subprocess.Popen(
            [binary], stdin=slave, stdout=slave, stderr=subprocess.PIPE,
            cwd=cwd, env=env, start_new_session=True, close_fds=True,
        )
        data = bytearray()
        sent = not keys
        sent_index = 0
        next_key_at = None
        started = time.monotonic()
        try:
            while time.monotonic() - started < LIMIT_SECONDS:
                ready, _, _ = select.select([master], [], [], 0.1)
                if ready:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        chunk = b""
                    data.extend(chunk)
                    if len(data) > LIMIT_BYTES:
                        raise AssertionError(f"{name}: PTY output cap exceeded")
                if not sent and b"Select a project" in data and next_key_at is None:
                    next_key_at = time.monotonic() + 0.25
                if not sent and next_key_at is not None and time.monotonic() >= next_key_at:
                    if burst:
                        os.write(master, keys)
                        sent = True
                    else:
                        os.write(master, keys[sent_index:sent_index + 1])
                        sent_index += 1
                        sent = sent_index == len(keys)
                        next_key_at = time.monotonic() + (0.30 if sent_index == len(keys) - 1 else 0.12)
                if child.poll() is not None:
                    break
            else:
                raise AssertionError(f"{name}: timeout")
            while True:
                ready, _, _ = select.select([master], [], [], 0)
                if not ready:
                    break
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                data.extend(chunk)
                if len(data) > LIMIT_BYTES:
                    raise AssertionError(f"{name}: PTY output cap exceeded")
            stderr = child.stderr.read(LIMIT_BYTES + 1)
            if len(stderr) > LIMIT_BYTES:
                raise AssertionError(f"{name}: stderr cap exceeded")
            if child.returncode != expected:
                raise AssertionError(f"{name}: exit {child.returncode}, expected {expected}")
            if marker is not None and marker not in data:
                raise AssertionError(f"{name}: selected value missing")
            if expected == 0 and b"? Select a project" not in data:
                raise AssertionError(f"{name}: stdout success line missing")
            if expected == 2 and data:
                raise AssertionError(f"{name}: CI gate wrote a prompt")
            if expected == 1 and data:
                raise AssertionError(f"{name}: failed prompt wrote to stdout")
            if b"/dev/tty" in stderr or stderr:
                raise AssertionError(f"{name}: unexpected stderr")
            if termios.tcgetattr(slave) != terminal_before:
                raise AssertionError(f"{name}: terminal mode was not restored")
            print(f"{name}: exit={child.returncode} stdout_bytes={len(data)} stderr_bytes=0")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=2)
            os.close(slave)
            os.close(master)


def probe_terminal_gate(binary: str, name: str, stdin_tty: bool, stdout_tty: bool) -> None:
    master, slave = pty.openpty()
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm", "NO_COLOR": "1"}
    with tempfile.TemporaryDirectory(prefix="c024f1-gate-") as cwd:
        child = subprocess.Popen(
            [binary], stdin=slave if stdin_tty else subprocess.PIPE,
            stdout=slave if stdout_tty else subprocess.PIPE,
            stderr=subprocess.PIPE, cwd=cwd, env=env,
            start_new_session=True, close_fds=True,
        )
        try:
            stdout, stderr = child.communicate(timeout=LIMIT_SECONDS)
            data = bytearray(stdout or b"")
            if stdout_tty:
                while select.select([master], [], [], 0)[0]:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        break
                    if not chunk:
                        break
                    data.extend(chunk)
                    if len(data) > LIMIT_BYTES:
                        raise AssertionError(f"{name}: PTY output cap exceeded")
            if child.returncode != 2 or data or stderr:
                raise AssertionError(f"{name}: gate did not exit 2 with empty output")
            print(f"{name}: exit=2 stdout_bytes=0 stderr_bytes=0")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=2)
            os.close(slave)
            os.close(master)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", help="absolute path to the c024f1_selector_probe example")
    args = parser.parse_args()
    alpha = b"SELECTED:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    zeta = b"SELECTED:bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"
    probe(args.binary, "default", b"\r", None, 0, alpha)
    probe(args.binary, "search", b"Zeta\r", None, 0, zeta)
    probe(args.binary, "burst-search", b"Zeta\r", None, 0, zeta, burst=True)
    probe(args.binary, "burst-interrupt", b"Z\x03", None, 130, None, burst=True)
    probe(args.binary, "uuid-fragment", b"bbbb\r", None, 0, zeta)
    probe(args.binary, "interrupt", b"\x03", None, 130, None)
    probe(args.binary, "ctrl-d", b"\x04", None, 3, None)
    probe(args.binary, "ci-empty", b"\r", "", 0, alpha)
    probe(args.binary, "ci-false", b"\r", "false", 0, alpha)
    probe(args.binary, "ci-zero", b"", "0", 2, None)
    probe(args.binary, "ci-pty", b"", "true", 2, None)
    probe(args.binary, "short-terminal", b"\r", None, 0, alpha, rows=3)
    probe(args.binary, "narrow-terminal", b"", None, 1, None, columns=3)
    probe(args.binary, "write-failure", b"", None, 1, None, fail_write=True)
    probe_terminal_gate(args.binary, "stdin-pipe", False, True)
    probe_terminal_gate(args.binary, "stdout-pipe", True, False)
    print("C024F1 PTY: 16/16 structural scenarios passed")


if __name__ == "__main__":
    main()
