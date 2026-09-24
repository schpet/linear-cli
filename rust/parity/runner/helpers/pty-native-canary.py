#!/usr/bin/env python3
"""Small, offline Linux proof of the PTY helper inside Bubblewrap.

Run with: python3 rust/parity/runner/helpers/pty-native-canary.py
The script stages and verifies the checked C helper with the Deno builder.
It does not use Cargo, case JSON, the host keyring, or an external network.
"""

import os
import re
import json
import hashlib
import select
import signal
import socket
import subprocess
import tempfile
import time
import uuid
from pathlib import Path

SOURCE = Path(__file__).with_name("status-helper.c")
NONCE = "a" * 32
IDENTITY = "b" * 32


def frame(conn):
    header = conn.recv(2)
    assert len(header) == 2, f"short status header: {header!r}"
    length = int.from_bytes(header, "big")
    payload = bytearray()
    while len(payload) < length:
        chunk = conn.recv(length - len(payload))
        assert chunk, "short status payload"
        payload.extend(chunk)
    return payload.decode("ascii")


def send_frame(conn, value):
    payload = value.encode("ascii")
    conn.sendall(len(payload).to_bytes(2, "big") + payload)


def pty_result(value, columns, rows):
    match = re.search(
        rf" pty {columns} {rows} (\d+) (raw|cooked|other) (\d+) (\d+) (\d+) (\d+) (\d+)$",
        value,
    )
    assert match, value
    assert int(match.group(6)) == columns and int(match.group(7)) == rows, value
    return int(match.group(1)), match.group(2)


def closing_frame(conn, name, expected_bytes):
    value = frame(conn)
    assert value == f"parity-status/3 {name} {NONCE} {expected_bytes}", value


def marker_pids(marker):
    found = []
    for path in Path("/proc").iterdir():
        if not path.name.isdecimal():
            continue
        try:
            if marker.encode() in (path / "cmdline").read_bytes():
                found.append(int(path.name))
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            pass
    return found


def host_pid_for_inner(marker, inner_pid):
    matched = []
    for pid in marker_pids(marker):
        try:
            status = Path(f"/proc/{pid}/status").read_text()
        except FileNotFoundError:
            continue
        nspid = re.search(r"^NSpid:\s+(.+)$", status, re.MULTILINE)
        if nspid and int(nspid.group(1).split()[-1]) == inner_pid:
            matched.append(pid)
    assert len(matched) == 1, (inner_pid, matched)
    return matched[0]


class Session:
    def __init__(self, helper, size, code, marker=None, target=None):
        self.listener = socket.socket()
        self.listener.bind(("127.0.0.1", 0))
        self.listener.listen(1)
        self.listener.settimeout(3)
        sandbox = Path(tempfile.mkdtemp(prefix="p04b-case-", dir="/var/tmp"))
        (sandbox / "tmp").mkdir()
        (sandbox / "cwd").mkdir()
        self.sandbox = sandbox
        command = [
            "/usr/bin/bwrap", "--unshare-user", "--unshare-pid",
            "--unshare-uts", "--hostname", "linear-parity", "--uid", "1000",
            "--gid", "1000", "--cap-drop", "ALL", "--disable-userns",
            "--assert-userns-disabled", "--die-with-parent", "--clearenv",
            "--setenv", "HOME", str(sandbox), "--setenv", "PATH", "",
            "--setenv", "DENO_NO_UPDATE_CHECK", "1",
            "--ro-bind", "/usr", "/usr", "--tmpfs", "/usr/local",
            "--remount-ro", "/usr/local",
            "--symlink", "usr/lib", "/lib",
            "--symlink", "usr/lib64", "/lib64",
            "--symlink", "usr/bin", "/bin",
            "--symlink", "usr/sbin", "/sbin",
            "--proc", "/proc", "--dev", "/dev",
            "--bind", str(sandbox), str(sandbox),
            "--bind", str(sandbox / "tmp"), "/tmp",
            "--ro-bind", str(helper), str(helper),
            "--remount-ro", "/", "--chdir", str(sandbox / "cwd"),
            "--", str(helper),
            str(self.listener.getsockname()[1]), NONCE, IDENTITY,
            "all-pty", size,
        ]
        command.extend(target or ["/usr/bin/python3", "-c", code])
        if marker is not None: command.append(marker)
        self.process = subprocess.Popen(
            command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True,
        )
        self.conn, _ = self.listener.accept()
        self.conn.settimeout(4)
        hello = frame(self.conn)
        assert hello.startswith(f"parity-status/3 HELLO {NONCE} {IDENTITY} "), hello
        send_frame(self.conn, f"parity-status/3 ACK {NONCE}")
        self.output = bytearray()

    def read_until(self, expected, seconds=3):
        deadline = time.monotonic() + seconds
        while expected not in self.output:
            remain = deadline - time.monotonic()
            assert remain > 0, (expected, bytes(self.output))
            ready, _, _ = select.select([self.process.stdout], [], [], remain)
            assert ready, (expected, bytes(self.output))
            chunk = os.read(self.process.stdout.fileno(), 4096)
            assert chunk, (expected, bytes(self.output))
            self.output.extend(chunk)

    def action(self, index, state, hex_key):
        self.process.stdin.write(f"ACTION {index} {state} {hex_key}\n".encode())
        self.process.stdin.flush()

    def frame_while_draining(self, seconds=4):
        deadline = time.monotonic() + seconds
        while True:
            remain = deadline - time.monotonic()
            assert remain > 0, bytes(self.output[-100:])
            ready, _, _ = select.select([self.conn, self.process.stdout], [], [], remain)
            assert ready, bytes(self.output[-100:])
            if self.process.stdout in ready:
                self.output.extend(os.read(self.process.stdout.fileno(), 4096))
            if self.conn in ready:
                return frame(self.conn)

    def finish(self, seconds=4):
        stdout, stderr = self.process.communicate(timeout=seconds)
        self.output.extend(stdout)
        import shutil
        shutil.rmtree(self.sandbox)
        return self.process.returncode, bytes(self.output), stderr

    def kill_outer(self, sig=signal.SIGKILL):
        os.killpg(self.process.pid, sig)


def main():
    with tempfile.TemporaryDirectory(prefix="p04b-stage-", dir="/var/tmp") as temp:
        expression = (
            'import {buildStatusHelper,verifyStatusHelper} from "./rust/parity/runner/helpers/build-status-helper.ts";'
            'const built=await buildStatusHelper({stageDir:Deno.args[0]});'
            'console.log(JSON.stringify(await verifyStatusHelper(built.path)))'
        )
        built = subprocess.run([
            "deno", "eval", "--frozen", "--config",
            "rust/parity/deno.json", expression, temp,
        ], cwd=SOURCE.parents[4], text=True, capture_output=True, check=True)
        artifact = json.loads(built.stdout)
        helper = Path(artifact["path"])
        assert artifact["sourceSha256"] == hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        assert artifact["binarySha256"] == hashlib.sha256(helper.read_bytes()).hexdigest()
        print(
            "verified provenance:",
            "source", artifact["sourceSha256"],
            "binary", artifact["binarySha256"],
            "compiler", artifact["compiler"]["sha256"],
            "kernel", os.uname().release,
        )

        invalid_size = subprocess.run([
            str(helper), "1", NONCE, IDENTITY, "all-pty", "0x10", "/usr/bin/true",
        ], text=True, capture_output=True)
        assert invalid_size.returncode == 120 and "invalid stdout count" in invalid_size.stderr
        print("noncanonical size rejected before HELLO: passed")

        for size, rows, columns in [("40x10", 10, 40), ("100x30", 30, 100)]:
            code = '''import os,sys,termios,fcntl,struct
print("TTY",*[int(os.isatty(i)) for i in (0,1,2)],flush=True)
print("SIZE",*struct.unpack("HHHH",fcntl.ioctl(0,termios.TIOCGWINSZ,bytes(8)))[:2],flush=True)
print("SESSION",os.getpid(),os.getsid(0),os.getpgrp(),os.tcgetpgrp(0),flush=True)
print("FDS",[i for i in range(3,32) if os.path.exists("/proc/self/fd/"+str(i))],flush=True)
tty=os.open("/dev/tty",os.O_RDONLY)
slave=os.open(os.ttyname(0),os.O_RDONLY)
print("DEVTTY",os.readlink("/proc/self/fd/0"),int(os.isatty(tty) and os.tcgetpgrp(tty)==os.tcgetpgrp(0)),int(os.fstat(slave).st_rdev==os.fstat(0).st_rdev),flush=True)
os.close(tty);os.close(slave)
t=termios.tcgetattr(0)
cc=[x[0] for x in t[6]]
expected=[0]*len(cc)
for k,v in ((termios.VINTR,3),(termios.VQUIT,28),(termios.VERASE,127),(termios.VKILL,21),(termios.VEOF,4),(termios.VMIN,1)):
 expected[k]=v
assert t[:4]==[termios.ICRNL,termios.OPOST|termios.ONLCR,termios.CS8|termios.CREAD,termios.ECHO|termios.ICANON|termios.ISIG|termios.IEXTEN],t
assert t[4:6]==[0,0] and cc==expected,t
print("BASE",*t[:6],"CC",bytes(cc).hex(),flush=True)
sys.stderr.write("ERR\\n");sys.stderr.flush()
sys.stdout.write("\\x1b]8;;https://example.com\\x1b\\\\link\\x1b]8;;\\x1b\\\\\\n");sys.stdout.flush()
print("PROMPT",flush=True)
t=termios.tcgetattr(0);t[3]&=~(termios.ECHO|termios.ICANON|termios.ISIG);termios.tcsetattr(0,termios.TCSANOW,t)
print("RAW",flush=True)
b=os.read(0,1);print("KEY",b.hex(),flush=True)
'''
            session = Session(helper, size, code)
            session.read_until(b"RAW\r\n")
            session.action(0, "raw", "79")
            applied = frame(session.conn)
            assert applied.startswith(f"parity-status/3 APPLIED {NONCE} 0 "), applied
            prefix_count = int(applied.rsplit(" ", 1)[1])
            assert prefix_count >= len(session.output), (prefix_count, session.output)
            result = frame(session.conn)
            assert f" exited 0 pty {columns} {rows} " in result, result
            status, output, diagnostic = session.finish()
            assert status == 0 and diagnostic == b"", (status, diagnostic)
            assert b"TTY 1 1 1\r\n" in output, output
            assert f"SIZE {rows} {columns}\r\n".encode() in output, output
            session_line = re.search(rb"SESSION (\d+) (\d+) (\d+) (\d+)\r\n", output)
            assert session_line and len(set(session_line.groups())) == 1, output
            assert b"FDS []\r\n" in output, output
            assert b"DEVTTY /dev/pts/0 1 1\r\n" in output and b"ERR\r\n" in output, output
            assert b"BASE 256 5 176 32779 0 0 CC " in output, output
            assert b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\r\n" in output
            assert b"KEY 79\r\n" in output
            baseline = re.search(rb"BASE 256 5 176 32779 0 0 CC [0-9a-f]+\r\n", output)
            assert baseline, output
            expected = (
                b"TTY 1 1 1\r\n" + f"SIZE {rows} {columns}\r\n".encode() +
                session_line.group(0) + b"FDS []\r\nDEVTTY /dev/pts/0 1 1\r\n" +
                baseline.group(0) + b"ERR\r\n" +
                b"\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\r\n" +
                b"PROMPT\r\nRAW\r\nKEY 79\r\n"
            )
            assert output == expected, (output, expected)
            result_bytes, state = pty_result(result, columns, rows)
            assert result_bytes <= len(output) and state == "raw", result
            closing_frame(session.conn, "DONE", len(output))
            print(size, "TTY/size/devtty/merged bytes/raw ACK/RESULT/EIO: passed")

        # A wrong termios gate must not acknowledge or write the key.
        gate = Session(helper, "40x10", 'import time;print("COOKED",flush=True);time.sleep(30)')
        gate.read_until(b"COOKED\r\n")
        gate.action(0, "raw", "79")
        gate.conn.settimeout(0.25)
        try:
            frame(gate.conn)
            raise AssertionError("wrong termios state was acknowledged")
        except socket.timeout:
            pass
        ready, _, _ = select.select([gate.process.stdout], [], [], 0.05)
        assert not ready, "wrong-gate key leaked to the cooked PTY as echo/output"
        gate.kill_outer()
        gate.finish()
        print("wrong raw/cooked gate: passed")

        cooked = Session(helper, "40x10", 'import signal,time;signal.signal(signal.SIGINT,signal.SIG_DFL);print("READY",flush=True);time.sleep(30)')
        cooked.read_until(b"READY\r\n")
        cooked.action(0, "cooked", "03")
        applied = frame(cooked.conn)
        assert applied.startswith(f"parity-status/3 APPLIED {NONCE} 0 "), applied
        result = frame(cooked.conn)
        assert " signaled 2 SIGINT pty 40 10 " in result, result
        status, output, diagnostic = cooked.finish()
        assert status == 130 and diagnostic == b"", (status, output, diagnostic)
        _, state = pty_result(result, 40, 10)
        assert state == "cooked", result
        closing_frame(cooked.conn, "DONE", len(output))
        print("cooked foreground Ctrl-C authenticated SIGINT: passed")

        realtime = Session(helper, "40x10", 'import os,signal;print("RTREADY",flush=True);os.kill(os.getpid(),signal.SIGRTMIN)')
        realtime.read_until(b"RTREADY\r\n")
        result = frame(realtime.conn)
        assert f" signaled-unlisted {signal.SIGRTMIN} pty 40 10 " in result, result
        status, output, diagnostic = realtime.finish()
        assert status == 128 + signal.SIGRTMIN and diagnostic == b"", (status, diagnostic)
        closing_frame(realtime.conn, "DONE", len(output))
        print("real-time signal yields typed target RESULT without helper stderr: passed")

        missing = Session(helper, "40x10", "", target=["/no/such/p04b-target"])
        try:
            missing.action(0, "cooked", "79")
        except BrokenPipeError:
            pass  # the missing target may already have been reaped
        failed = frame(missing.conn)
        assert " exec-failed 2" in failed, failed
        status, _, diagnostic = missing.finish()
        assert status == 124 and b"PTY execve" in diagnostic, (status, diagnostic)
        closing_frame(missing.conn, "DONE", len(missing.output))
        print("target exec failure is a harness result: passed")

        reopen_code = '''import os,time
name=os.ttyname(0)
print("BEFORE",flush=True)
for fd in (0,1,2): os.close(fd)
time.sleep(0.1)
slave=os.open(name,os.O_RDWR|os.O_NOCTTY)
for fd in (0,1,2): os.dup2(slave,fd)
if slave>2: os.close(slave)
print("REOPEN",flush=True)
'''
        reopened = Session(helper, "40x10", reopen_code)
        result = frame(reopened.conn)
        assert " exited 0 pty 40 10 " in result, result
        status, output, diagnostic = reopened.finish()
        assert status == 0 and output == b"BEFORE\r\nREOPEN\r\n" and diagnostic == b"", (status, output, diagnostic)
        closing_frame(reopened.conn, "DONE", len(output))
        print("pre-reap EIO followed by reopened slave: passed")

        altered_terminal = '''import fcntl,struct,termios
fcntl.ioctl(0,termios.TIOCSWINSZ,struct.pack("HHHH",20,70,0,0))
t=termios.tcgetattr(0);t[3]&=~(termios.ECHO|termios.ICANON|termios.ISIG);termios.tcsetattr(0,termios.TCSANOW,t)
print("ALTERED",flush=True)
'''
        changed = Session(helper, "40x10", altered_terminal)
        result = frame(changed.conn)
        match = re.search(r" pty 40 10 (\d+) raw (\d+) (\d+) (\d+) 70 20$", result)
        assert match, result
        assert int(match.group(4)) & 11 == 0, result  # ECHO, ICANON, ISIG off
        status, output, diagnostic = changed.finish()
        assert status == 0 and output == b"ALTERED\r\n" and diagnostic == b"", (status, output, diagnostic)
        closing_frame(changed.conn, "DONE", len(output))
        print("final raw termios and observed resized winsize: passed")

        close_before_key = '''import os,time
print("READY",flush=True)
for fd in (0,1,2): os.close(fd)
time.sleep(0.1)
'''
        closed = Session(helper, "40x10", close_before_key)
        closed.read_until(b"READY\r\n")
        try:
            closed.action(0, "cooked", "79")
        except BrokenPipeError:
            pass
        result = frame(closed.conn)
        assert result.startswith(f"parity-status/3 RESULT {NONCE} {IDENTITY} "), result
        assert " exited 0 pty 40 10 " in result, result  # first frame is RESULT, never APPLIED
        status, output, diagnostic = closed.finish()
        assert status == 0 and output == b"READY\r\n" and diagnostic == b"", (status, output, diagnostic)
        closing_frame(closed.conn, "DONE", len(output))
        print("target EOF leaves pending action unapplied without helper stderr: passed")

        # Every key gets a fresh living target. Rejection must end the helper
        # before any APPLIED/RESULT/DONE frame or target read can occur.
        def raw_reader(length):
            return f'''import os,termios
t=termios.tcgetattr(0);t[3]&=~(termios.ECHO|termios.ICANON|termios.ISIG);termios.tcsetattr(0,termios.TCSANOW,t)
print("READY",flush=True)
key=b""
while len(key)<{length}: key+=os.read(0,{length}-len(key))
print("KEY",key.hex(),flush=True)
'''

        for rejected in ("00", "1b"):
            session = Session(helper, "40x10", raw_reader(1))
            session.read_until(b"READY\r\n")
            session.action(0, "raw", rejected)
            status, output, diagnostic = session.finish()
            assert status == 122 and b"not one key event" in diagnostic, (rejected, status, diagnostic)
            assert output == b"READY\r\n", (rejected, output)
            assert session.conn.recv(1) == b"", f"{rejected}: unexpected status frame"
            print(f"raw key {rejected} rejected before APPLIED/RESULT/DONE: passed")

        for accepted, length in (("01", 1), ("1c", 1), ("7f", 1), ("1b5b41", 3)):
            session = Session(helper, "40x10", raw_reader(length))
            session.read_until(b"READY\r\n")
            session.action(0, "raw", accepted)
            applied = frame(session.conn)
            assert applied == f"parity-status/3 APPLIED {NONCE} 0 7", (accepted, applied)
            result = frame(session.conn)
            assert " exited 0 pty 40 10 " in result, (accepted, result)
            status, output, diagnostic = session.finish()
            assert status == 0 and diagnostic == b"", (accepted, status, diagnostic)
            assert output == b"READY\r\nKEY " + accepted.encode() + b"\r\n", (accepted, output)
            closing_frame(session.conn, "DONE", len(output))
            assert session.conn.recv(1) == b"", f"{accepted}: unexpected trailing status frame"
            print(f"raw key {accepted} APPLIED and exact target read: passed")

        invalid = Session(helper, "40x10", 'import time;print("READY",flush=True);time.sleep(30)')
        invalid.read_until(b"READY\r\n")
        invalid.action(0, "cooked", "796e")  # two ASCII keys, not one event
        status, _, diagnostic = invalid.finish()
        assert status == 122 and b"not one key event" in diagnostic, (status, diagnostic)
        print("concatenated key rejection: passed")

        # The second identical prompt is fresh only after the first ACK's
        # relayed-byte boundary. No old prompt may trigger the next key.
        repeated = '''import os,termios
t=termios.tcgetattr(0);t[3]&=~(termios.ECHO|termios.ICANON|termios.ISIG);termios.tcsetattr(0,termios.TCSANOW,t)
for _ in range(2):
 print("PROMPT",flush=True)
 print("KEY",os.read(0,1).hex(),flush=True)
'''
        session = Session(helper, "40x10", repeated)
        session.read_until(b"PROMPT\r\n")
        session.action(0, "raw", "79")
        first = frame(session.conn)
        assert first.endswith(" 8"), first  # first PROMPT plus CRLF
        session.read_until(b"PROMPT\r\nKEY 79\r\nPROMPT\r\n")
        session.action(1, "raw", "6e")
        second = frame(session.conn)
        assert int(second.rsplit(" ", 1)[1]) >= len(session.output), second
        assert " APPLIED " in first and " APPLIED " in second
        result = frame(session.conn)
        assert " exited 0 pty 40 10 " in result, result
        status, output, diagnostic = session.finish()
        assert status == 0 and diagnostic == b"" and output.endswith(b"KEY 6e\r\n")
        closing_frame(session.conn, "DONE", len(output))
        print("repeated prompt ACK boundary: passed")

        render_then_raw = '''import os,time,termios
for index in range(2):
 os.write(1,b"PROMPT\\n")
 time.sleep(0.06)
 os.write(1,b"HINT\\n")
 t=termios.tcgetattr(0);t[3]&=~(termios.ECHO|termios.ICANON|termios.ISIG);termios.tcsetattr(0,termios.TCSANOW,t)
 key=os.read(0,8)
 termios.tcsetattr(0,termios.TCSANOW,t[:3]+[t[3]|termios.ECHO|termios.ICANON|termios.ISIG]+t[4:])
 print("KEY",index,key.hex(),flush=True)
'''
        session = Session(helper, "40x10", render_then_raw)
        session.read_until(b"PROMPT\r\n")
        session.action(0, "raw", "79")
        first = frame(session.conn)
        assert first == f"parity-status/3 APPLIED {NONCE} 0 14", first
        session.read_until(b"KEY 0 79\r\nPROMPT\r\nHINT\r\n")
        session.action(1, "raw", "6e")
        second = frame(session.conn)
        assert int(second.rsplit(" ", 1)[1]) == len(session.output), second
        assert int(second.rsplit(" ", 1)[1]) > 14, second
        result = frame(session.conn)
        status, output, diagnostic = session.finish()
        assert status == 0 and diagnostic == b"", (status, diagnostic)
        assert output == b"PROMPT\r\nHINT\r\nKEY 0 79\r\nPROMPT\r\nHINT\r\nKEY 1 6e\r\n", output
        pty_result(result, 40, 10)
        closing_frame(session.conn, "DONE", len(output))
        print("render-then-raw repeated hint ACK exact boundary: passed")

        # Simulate runner deadline, cap, abort and outer SIGINT with a grandchild
        # that escaped the target's session and retained the slave.
        code = '''import subprocess,time,sys
child="import os,time;print('ESCAPED',os.getpid(),os.getsid(0),os.getpgrp(),os.ttyname(0),flush=True);time.sleep(30)"
p=subprocess.Popen(["/usr/bin/python3","-c",child,sys.argv[1]],start_new_session=True)
print("SPAWN",p.pid,flush=True)
time.sleep(30)
'''
        for event in ("deadline", "cap", "abort", "SIGINT"):
            marker = "p04b-" + uuid.uuid4().hex
            event_code = code.replace(
                "time.sleep(30)\n",
                'import os\nwhile True: os.write(1,b"x"*4096)\n',
            ) if event == "cap" else code
            session = Session(helper, "40x10", event_code, marker)
            session.read_until(b"ESCAPED ")
            session.read_until(b"/dev/pts/0\r\n")
            if event == "cap":
                session.read_until(b"x" * 1024)
            spawned = re.search(rb"SPAWN (\d+)\r\n", session.output)
            escaped = re.search(rb"ESCAPED (\d+) (\d+) (\d+) (/dev/pts/\d+)\r\n", session.output)
            assert spawned and escaped, (event, bytes(session.output))
            inner_pid = int(spawned.group(1))
            assert inner_pid == int(escaped.group(1)) == int(escaped.group(2)) == int(escaped.group(3))
            host_pid = host_pid_for_inner(marker, inner_pid)
            assert Path(f"/proc/{host_pid}/fd/0").resolve().as_posix() == escaped.group(4).decode()
            session.kill_outer(signal.SIGINT if event == "SIGINT" else signal.SIGKILL)
            session.finish()
            for _ in range(80):
                if not Path(f"/proc/{host_pid}").exists():
                    break
                time.sleep(0.025)
            assert not Path(f"/proc/{host_pid}").exists(), (event, host_pid)
            print(event, "escaped grandchild session/PTY cleanup: passed")

        # Keep the actual target short-lived; its grandchild retains the slave.
        retained = code.replace("\ntime.sleep(30)\n", "\ntime.sleep(0.2)\nsys.exit(122)\n")
        marker = "p04b-" + uuid.uuid4().hex
        session = Session(helper, "40x10", retained, marker)
        session.read_until(b"ESCAPED ")
        session.read_until(b"/dev/pts/0\r\n")
        result = frame(session.conn)
        assert " exited 122 pty 40 10 " in result, result
        retained_frame = frame(session.conn)
        assert retained_frame.startswith(f"parity-status/3 RETAINED {NONCE} "), retained_frame
        status, _, diagnostic = session.finish()
        assert status == 122 and diagnostic == b"", (status, diagnostic)
        assert int(retained_frame.rsplit(" ", 1)[1]) == len(session.output)
        for _ in range(80):
            if not marker_pids(marker):
                break
            time.sleep(0.025)
        assert not marker_pids(marker), marker_pids(marker)
        print("retained slave after exact RESULT: bounded failure and cleanup passed")

        continuous = '''import subprocess,time,sys
child="import os,time;print('ESCAPED',os.getpid(),os.getsid(0),os.getpgrp(),os.ttyname(0),flush=True)\\nwhile True:\\n os.write(1,b'x'*1024);time.sleep(0.01)"
p=subprocess.Popen(["/usr/bin/python3","-c",child,sys.argv[1]],start_new_session=True)
print("SPAWN",p.pid,flush=True)
time.sleep(0.2)
'''
        marker = "p04b-" + uuid.uuid4().hex
        session = Session(helper, "40x10", continuous, marker)
        session.read_until(b"ESCAPED ")
        spawned = re.search(rb"SPAWN (\d+)\r\n", session.output)
        assert spawned, bytes(session.output)
        host_pid = host_pid_for_inner(marker, int(spawned.group(1)))
        result = session.frame_while_draining()
        assert " exited 0 pty 40 10 " in result, result
        retained_frame = session.frame_while_draining()
        assert retained_frame.startswith(f"parity-status/3 RETAINED {NONCE} "), retained_frame
        status, output, diagnostic = session.finish()
        assert status == 0 and diagnostic == b"", (status, diagnostic)
        assert len(output) > 64 * 1024, len(output)
        assert int(retained_frame.rsplit(" ", 1)[1]) == len(output), (retained_frame, len(output))
        for _ in range(80):
            if not Path(f"/proc/{host_pid}").exists(): break
            time.sleep(0.025)
        assert not Path(f"/proc/{host_pid}").exists(), host_pid
        print("continuous post-exit writer bounded RETAINED and cleanup: passed")

        flood = Session(helper, "40x10", 'import os\nwhile True: os.write(1,b"x"*16384)')
        limited = flood.frame_while_draining(seconds=20)
        assert limited.startswith(f"parity-status/3 LIMIT {NONCE} "), limited
        status, output, diagnostic = flood.finish(seconds=5)
        assert status == 126 and diagnostic == b"", (status, diagnostic)
        assert int(limited.rsplit(" ", 1)[1]) == len(output) > 64 * 1024 * 1024
        print("native relay guard emits authenticated LIMIT without stderr: passed")


if __name__ == "__main__":
    main()
