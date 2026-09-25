"""Bounded, C024-only PTY/oracle probe; never reads host credentials.

Usage: python3 rust/parity/runner/c024-terminal-probe.py --inside
The outer invocation enters a fresh user/network/PID namespace, then the inner
process uses fake HOME/config/PATH, a localhost GraphQL server and PTYs. It
prints synthetic-data digests and process traces as JSON. This probe does not
alter the strict pipe-mode case schema or make a route-complete parity claim.
"""

from __future__ import annotations

from copy import deepcopy
from datetime import datetime, timezone
import fcntl
import hashlib
import http.server
import json
import os
from pathlib import Path
import pty
import re
import select
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time

ROOT = Path(__file__).resolve().parents[3]
CASES = ROOT / "rust/parity/runner/c024-frozen-cases"
REF = Path("/home/exedev/workspace/linear-cli-rust-deno-reference")
BIN = Path("/home/exedev/workspace/linear-cli/untracked/notebook/2026-09-23-rust-port/P01/reference-linear")
DENO = Path("/home/exedev/.deno/bin/deno")
STAGE = Path("/home/exedev/.cache/linear-parity/stage/3da729da08fe6d48/deno")
SOURCE_SHA256 = "4f9832d6cb912cf7a545e8fd40133e678dcca99ea2a77ea1c82c05c23396f430"
BINARY_SHA256 = "a17675c5ab9a0bf5f32f65e5e68112676576972a9979f5a97bc844f6b23e0835"
LOCK_SHA256 = "3da729da08fe6d48236e055b2eaac95788b5e5ccfd0f66dacdc5f6a0b0b96403"
REFERENCE_REVISION = "d4fe6fa7358f018fd1da0c6b96ec2b022247e898"
MAX_OUTPUT = 2 * 1024 * 1024
TIMEOUT = 25.0


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_query(value: str) -> str:
    return " ".join(value.split())


def same_json(left: object, right: object) -> bool:
    return json.dumps(left, sort_keys=True, separators=(",", ":")) == json.dumps(
        right, sort_keys=True, separators=(",", ":")
    )


def load_case(name: str) -> dict:
    return json.loads((CASES / f"c024-{name}.json").read_text())


class FixtureServer(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, steps: list[dict]):
        self.steps = steps
        self.requests: list[dict] = []
        self.issues: list[str] = []
        super().__init__(("127.0.0.1", 0), Handler)


class Handler(http.server.BaseHTTPRequestHandler):
    server: FixtureServer

    def log_message(self, _format: str, *_args: object) -> None:
        pass

    def do_POST(self) -> None:
        server = self.server
        length = int(self.headers.get("content-length", "0"))
        if self.path != "/graphql" or length > MAX_OUTPUT:
            server.issues.append("bad path or body size")
            self.send_error(400)
            return
        try:
            request = json.loads(self.rfile.read(length))
        except (ValueError, TypeError):
            server.issues.append("invalid request JSON")
            self.send_error(400)
            return
        index = len(server.requests)
        server.requests.append({
            "query": canonical_query(request.get("query", "")),
            "variables": request.get("variables", {}),
            "authorization": self.headers.get("authorization"),
            "userAgent": self.headers.get("user-agent"),
        })
        if index >= len(server.steps):
            server.issues.append("extra GraphQL request")
            self.send_error(400)
            return
        expected = server.steps[index]
        if not same_json(server.requests[index], {
            "query": canonical_query(expected["operation"]["document"]),
            "variables": expected["operation"].get("variables", {}),
            "authorization": "lin_api_fake",
            "userAgent": "schpet-linear-cli/2.6.0",
        }):
            server.issues.append(f"request {index + 1} differs")
        response = expected["response"]
        if response["kind"] != "data":
            server.issues.append("terminal probe requires data response")
            self.send_error(500)
            return
        body = json.dumps({"data": response["data"]}, separators=(",", ":")).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def make_step(name: str, variables: dict, data: dict) -> dict:
    source = (REF / "src/commands/project/project-view.ts").read_text()
    match = re.search(r"query " + re.escape(name) + r"\b.*?\n\s*`", source, re.S)
    if match is None:
        raise RuntimeError(f"missing frozen query {name}")
    return {"operation": {"document": match.group()[:-1], "variables": variables},
            "response": {"kind": "data", "data": data}}


def scenario(name: str) -> tuple[list[str], list[dict], bool, str | None, int, int, str | None]:
    minimal = load_case("minimal-json")["graphql"]["groups"][0]["steps"][0]
    rich = load_case("rich-json")["graphql"]["groups"][0]["steps"][0]
    uuid = minimal["operation"]["variables"]["id"]
    if name == "minimal-tty":
        return ["project", "view", uuid], [minimal], False, None, 80, 50, "1"
    if name == "rich-narrow":
        return ["project", "view", uuid], [rich], False, None, 40, 100, "1"
    if name == "rich-wide":
        return ["project", "view", uuid], [rich], False, None, 120, 100, "1"
    if name == "rich-color-image":
        colored = deepcopy(rich)
        colored["response"]["data"]["project"]["content"] = "![Diagram](https://example.com/image.png)"
        return ["project", "view", uuid], [colored], False, None, 80, 100, None
    if name == "no-color-empty":
        return ["project", "view", uuid], [minimal], False, None, 80, 50, ""
    if name == "pager":
        return ["project", "view", uuid], [rich], False, "pager-probe -R", 80, 8, "1"
    if name == "pager-fallback":
        return ["project", "view", uuid], [rich], False, "pager-fail -R", 80, 8, "1"
    if name == "no-pager":
        return ["project", "view", uuid, "--no-pager"], [rich], False, "pager-probe -R", 80, 8, "1"
    if name == "web":
        return ["project", "view", uuid, "--web"], [], False, None, 80, 30, "1"
    if name == "app-web":
        return ["project", "view", uuid, "--app", "--web"], [], False, None, 80, 30, "1"
    if name == "picker":
        p1 = make_step("GetProjectsForPicker", {"first": 100}, {
            "projects": {"nodes": [{"id": uuid, "name": "Simple Project", "slugId": "simple-abc123def456",
                                    "status": {"name": "Backlog"}, "teams": {"nodes": [{"key": "ENG"}]}}],
                         "pageInfo": {"hasNextPage": True, "endCursor": "next"}}})
        p2 = make_step("GetProjectsForPicker", {"first": 100, "after": "next"}, {
            "projects": {"nodes": [], "pageInfo": {"hasNextPage": False, "endCursor": None}}})
        return ["project", "view"], [p1, p2, minimal], True, None, 80, 40, "1"
    if name == "picker-search":
        zeta_id = "00000000-0000-0000-0000-000000000001"
        projects = [
            {"id": zeta_id, "name": "Zeta Project",
             "slugId": "zeta-abc123def456", "status": {"name": "Backlog"},
             "teams": {"nodes": [{"key": "OPS"}]}},
            {"id": uuid, "name": "Simple Project", "slugId": "simple-abc123def456",
             "status": {"name": "Backlog"}, "teams": {"nodes": [{"key": "ENG"}]}},
        ]
        picker_step = make_step("GetProjectsForPicker", {"first": 100}, {
            "projects": {"nodes": projects,
                         "pageInfo": {"hasNextPage": False, "endCursor": None}}})
        selected = deepcopy(minimal)
        selected["operation"]["variables"]["id"] = zeta_id
        selected["response"]["data"]["project"]["id"] = zeta_id
        selected["response"]["data"]["project"]["name"] = "Zeta Project"
        selected["response"]["data"]["project"]["slugId"] = "zeta-abc123def456"
        selected["response"]["data"]["project"]["url"] = "https://linear.app/alpha/project/zeta-abc123def456"
        return ["project", "view"], [picker_step, selected], True, None, 80, 40, "1"
    raise RuntimeError(f"unknown scenario {name}")


def make_bin(directory: Path) -> None:
    bin_dir = directory / "bin"
    bin_dir.mkdir()
    scripts = {
        "xdg-open": '#!/bin/sh\nprintf "opener-argc:%s\\n" "$#" >> "$C024_TRACE"\nfor arg in "$@"; do printf "opener-arg:%s\\n" "$arg" >> "$C024_TRACE"; done\nexit 0\n',
        "pager-probe": '#!/bin/sh\n/bin/cat > "$C024_PAGER_CAPTURE"\nprintf "pager-argc:%s\\n" "$#" >> "$C024_TRACE"\nfor arg in "$@"; do printf "pager-arg:%s\\n" "$arg" >> "$C024_TRACE"; done\nexit 0\n',
        "pager-fail": '#!/bin/sh\n/bin/cat > /dev/null\nprintf "pager-fail-argc:%s\\n" "$#" >> "$C024_TRACE"\nfor arg in "$@"; do printf "pager-fail-arg:%s\\n" "$arg" >> "$C024_TRACE"; done\nexit 1\n',
        "less": '#!/bin/sh\n/bin/cat > "$C024_PAGER_CAPTURE"\nprintf "less-argc:%s\\n" "$#" >> "$C024_TRACE"\nfor arg in "$@"; do printf "less-arg:%s\\n" "$arg" >> "$C024_TRACE"; done\nexit 0\n',
    }
    for name, body in scripts.items():
        path = bin_dir / name
        path.write_text(body)
        path.chmod(0o755)


def drive(label: str, name: str, directory: Path) -> dict:
    args, steps, picker, pager, columns, rows, no_color = scenario(name)
    server = FixtureServer(steps)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        work = directory / f"{label}-{name}"
        work.mkdir()
        home = work / "home"
        home.mkdir()
        config = work / "config"
        config.mkdir()
        make_bin(work)
        if name in {"web", "app-web"}:
            (work / "linear.toml").write_text('workspace = "alpha"\n')
        if name == "rich-color-image":
            (work / "linear.toml").write_text('hyperlink_format = "default"\n')
        trace = work / "trace"
        capture = work / "pager-bytes"
        env = {
            "HOME": str(home), "XDG_CONFIG_HOME": str(config), "APPDATA": str(config),
            "PATH": str(work / "bin"), "DENO_DIR": str(STAGE), "DENO_NO_UPDATE_CHECK": "1",
            "TZ": "UTC", "LANG": "C.UTF-8", "LINEAR_IGNORE_ENV_FILE": "1",
            "LINEAR_GRAPHQL_ENDPOINT": f"http://127.0.0.1:{server.server_port}/graphql",
            "LINEAR_API_KEY": "lin_api_fake", "C024_TRACE": str(trace),
            "C024_PAGER_CAPTURE": str(capture),
        }
        if no_color is not None:
            env["NO_COLOR"] = no_color
        if pager is not None:
            env["PAGER"] = pager
        if label == "compiled":
            command = [str(BIN), *args]
        else:
            command = [str(DENO), "run", "--cached-only", "--frozen", "--allow-all",
                       "--quiet", "--config", str(REF / "deno.json"), str(REF / "src/main.ts"), *args]
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        proc = subprocess.Popen(command, cwd=work, env=env, stdin=slave, stdout=slave,
                                stderr=subprocess.PIPE, start_new_session=True)
        os.close(slave)
        output = bytearray()
        error_output = bytearray()
        stderr_open = proc.stderr is not None
        sent = False
        search_index = 0
        next_search_key = 0.0
        search_keys = b"Zeta\r"
        start = time.monotonic()
        try:
            while True:
                if time.monotonic() - start > TIMEOUT:
                    raise RuntimeError(
                        f"{label}/{name}: timeout; synthetic PTY tail {bytes(output[-500:])!r}"
                    )
                watched = [master]
                if stderr_open:
                    watched.append(proc.stderr)
                ready, _, _ = select.select(watched, [], [], 0.1)
                ended = False
                if master in ready:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError:
                        chunk = b""
                    if not chunk:
                        ended = True
                    output.extend(chunk)
                    if len(output) > MAX_OUTPUT:
                        raise RuntimeError(f"{label}/{name}: output cap")
                if stderr_open and proc.stderr in ready:
                    chunk = os.read(proc.stderr.fileno(), 65536)
                    if chunk:
                        error_output.extend(chunk)
                        if len(error_output) > MAX_OUTPUT:
                            raise RuntimeError(f"{label}/{name}: stderr cap")
                    else:
                        stderr_open = False
                if picker and not sent and b"Select a project" in output:
                    if name == "picker-search":
                        if time.monotonic() >= next_search_key:
                            os.write(master, search_keys[search_index:search_index + 1])
                            search_index += 1
                            sent = search_index == len(search_keys)
                            next_search_key = time.monotonic() + (
                                0.4 if search_index == len(search_keys) - 1 else 0.12
                            )
                    else:
                        os.write(master, b"\r")
                        sent = True
                if proc.poll() is not None and (ended or master not in ready):
                    break
        finally:
            if proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait()
            if proc.stderr is not None:
                proc.stderr.close()
            os.close(master)
        if len(server.requests) != len(steps) or server.issues:
            raise RuntimeError(f"{label}/{name}: request mismatch {server.issues}; {len(server.requests)}/{len(steps)}")
        pager_bytes = capture.read_bytes() if capture.exists() else None
        return {"reference": label, "scenario": name, "exit": proc.returncode,
                "columns": columns, "rows": rows,
                "noColor": "absent" if no_color is None else ("empty" if no_color == "" else "set"),
                "stdoutSha256": sha(bytes(output)), "stdoutBytes": len(output),
                "stdoutOsc8Count": output.count(b"\x1b]8;;"),
                "stdoutCsiCount": len(re.findall(rb"\x1b\[[0-9;]*[A-Za-z]", output)),
                "stdoutSpinnerClearCount": output.count(b"\r\x1b[K"),
                "stderrSha256": sha(bytes(error_output)), "stderrBytes": len(error_output),
                "requests": len(server.requests), "pickerEnterSent": sent,
                "processTrace": trace.read_text().splitlines() if trace.exists() else [],
                "pagerInputSha256": sha(pager_bytes) if pager_bytes is not None else None,
                "pagerInputBytes": len(pager_bytes) if pager_bytes is not None else None,
                "pagerInputLastByteHex": pager_bytes[-1:].hex() if pager_bytes else None}
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def main() -> None:
    if sys.argv[1:] != ["--inside"]:
        revision = subprocess.run(
            ["jj", "log", "-r", "@-", "--no-graph", "-T", "commit_id"],
            cwd=REF, capture_output=True, text=True, check=True,
        ).stdout.strip()
        if revision != REFERENCE_REVISION:
            raise RuntimeError("interpreted reference revision changed")
        changed = subprocess.run(
            ["jj", "diff", "-r", "@", "--summary"],
            cwd=REF, capture_output=True, text=True, check=True,
        ).stdout.strip()
        if changed:
            raise RuntimeError("interpreted reference working copy changed")
        result = subprocess.run(["/bin/unshare", "--user", "--map-root-user", "--net",
                                 "--pid", "--fork", "--mount-proc", sys.executable,
                                 str(Path(__file__).resolve()), "--inside"], check=False)
        raise SystemExit(result.returncode)
    subprocess.run(["/bin/ip", "link", "set", "lo", "up"], check=True)
    with socket.socket() as probe:
        probe.settimeout(1)
        if probe.connect_ex(("198.51.100.1", 443)) == 0:
            raise RuntimeError("network namespace unexpectedly reached a non-loopback address")
    if not (BIN.is_file() and REF.is_dir() and STAGE.is_dir()):
        raise RuntimeError("pinned reference or staged Deno cache missing")
    if sha(BIN.read_bytes()) != BINARY_SHA256:
        raise RuntimeError("compiled reference changed")
    if sha((REF / "src/commands/project/project-view.ts").read_bytes()) != SOURCE_SHA256:
        raise RuntimeError("interpreted project-view source changed")
    if sha((REF / "deno.lock").read_bytes()) != LOCK_SHA256:
        raise RuntimeError("interpreted reference lock changed")
    scenarios = ["minimal-tty", "rich-narrow", "rich-wide", "rich-color-image",
                 "no-color-empty", "pager", "pager-fallback", "no-pager",
                 "web", "app-web", "picker", "picker-search"]
    selected = os.environ.get("C024_SCENARIO")
    if selected is not None:
        if selected not in scenarios:
            raise RuntimeError("unknown C024_SCENARIO")
        scenarios = [selected]
    results = []
    with tempfile.TemporaryDirectory(prefix="c024-terminal-") as raw_dir:
        directory = Path(raw_dir)
        for name in scenarios:
            for label in ("interpreted", "compiled"):
                results.append(drive(label, name, directory))
    print(json.dumps({"networkNamespace": True, "syntheticOnly": True,
                      "referenceRevision": REFERENCE_REVISION,
                      "sourceSha256": SOURCE_SHA256,
                      "lockSha256": LOCK_SHA256,
                      "binarySha256": BINARY_SHA256,
                      "probeSha256": sha(Path(__file__).read_bytes()),
                      "capturedAtUtc": datetime.now(timezone.utc).isoformat(),
                      "scenarios": len(scenarios), "processes": len(results),
                      "results": results}, indent=2))


if __name__ == "__main__":
    main()
