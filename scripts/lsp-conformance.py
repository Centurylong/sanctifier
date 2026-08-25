#!/usr/bin/env python3
"""End-to-end conformance check for the Sanctifier language server.

The unit tests in `tooling/sanctifier-lsp` drive `serve()` over in-memory
buffers. This drives the real binary over real pipes, which is the only place a
framing or flushing bug actually shows up: an in-memory `Cursor` never blocks,
so a missing flush or a miscounted `Content-Length` looks fine there and hangs a
real editor.

It also measures the edit-to-diagnostics latency that issue #138 puts a 500ms
budget on, using a genuinely large source file rather than a toy fixture.

Usage:  scripts/lsp-conformance.py path/to/sanctifier-lsp
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import time

LATENCY_BUDGET_MS = 500
EDIT_ROUNDS = 5

VULNERABLE = (
    "use soroban_sdk::{contract, contractimpl, Address, Env};\n"
    "\n"
    "#[contract]\n"
    "pub struct Vault;\n"
    "\n"
    "#[contractimpl]\n"
    "impl Vault {\n"
    "    pub fn withdraw(env: Env, from: Address, amount: i128) {\n"
    "        let balance: i128 = env.storage().persistent().get(&from).unwrap();\n"
    "        env.storage().persistent().set(&from, &(balance - amount));\n"
    "    }\n"
    "}\n"
)

failures: list[str] = []


def check(label: str, condition: bool, detail: str = "") -> None:
    status = "PASS" if condition else "FAIL"
    print(f"  [{status}] {label}" + (f" — {detail}" if detail else ""))
    if not condition:
        failures.append(label)


def frame(payload: dict) -> bytes:
    body = json.dumps(payload).encode()
    return b"Content-Length: %d\r\n\r\n" % len(body) + body


def read_frame(stream) -> dict | None:
    """Read one LSP frame. Returns None at clean end of stream."""
    length = None
    while True:
        line = stream.readline()
        if not line:
            return None
        text = line.decode().strip()
        if text == "":
            break
        name, _, value = text.partition(":")
        if name.strip().lower() == "content-length":
            length = int(value.strip())
    if length is None:
        raise AssertionError("server sent a frame with no Content-Length")
    return json.loads(stream.read(length))


def did_open(uri: str, text: str) -> dict:
    return {
        "jsonrpc": "2.0",
        "method": "textDocument/didOpen",
        "params": {
            "textDocument": {
                "uri": uri,
                "languageId": "rust",
                "version": 1,
                "text": text,
            }
        },
    }


def did_change(uri: str, text: str, version: int) -> dict:
    return {
        "jsonrpc": "2.0",
        "method": "textDocument/didChange",
        "params": {
            "textDocument": {"uri": uri, "version": version},
            "contentChanges": [{"text": text}],
        },
    }


def large_fixture() -> tuple[str, str]:
    """A real source file, so the latency number means something."""
    here = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(here, "..", "tooling", "sanctifier-core", "src", "lib.rs"),
        os.path.join(here, "..", "tooling", "sanctifier-core", "src", "rules.rs"),
    ]
    for path in candidates:
        if os.path.exists(path) and os.path.getsize(path) > 2000:
            with open(path, encoding="utf-8") as handle:
                return os.path.relpath(path, here), handle.read()
    # Nothing large available — fall back so the check still runs.
    return "<synthetic>", VULNERABLE * 200


def run_session(binary: str) -> None:
    print("== session: initialize, diagnostics, hover, shutdown ==")
    server = subprocess.Popen(
        [binary, "--stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    def send(payload: dict) -> None:
        server.stdin.write(frame(payload))
        server.stdin.flush()

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
    initialize = read_frame(server.stdout)
    check("initialize is answered", initialize is not None and initialize.get("id") == 1)
    capabilities = (initialize or {}).get("result", {}).get("capabilities", {})
    check("hover capability is advertised", capabilities.get("hoverProvider") is True)
    check("text sync capability is advertised", isinstance(capabilities.get("textDocumentSync"), dict))

    send({"jsonrpc": "2.0", "method": "initialized", "params": {}})

    uri = "file:///vault.rs"
    send(did_open(uri, VULNERABLE))
    published = read_frame(server.stdout)
    check(
        "diagnostics are published on open",
        published is not None and published.get("method") == "textDocument/publishDiagnostics",
    )
    diagnostics = (published or {}).get("params", {}).get("diagnostics", [])
    check("a vulnerable contract yields diagnostics", len(diagnostics) > 0, f"{len(diagnostics)} reported")
    check(
        "diagnostics are attributed to sanctifier",
        all(d.get("source") == "sanctifier" for d in diagnostics),
    )
    check(
        "diagnostic ranges are inside the document",
        all(d["range"]["start"]["line"] < len(VULNERABLE.splitlines()) for d in diagnostics),
    )

    # Hover on a line the server reported a finding for.
    target_line = diagnostics[0]["range"]["start"]["line"] if diagnostics else 0
    send(
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "textDocument/hover",
            "params": {"textDocument": {"uri": uri}, "position": {"line": target_line, "character": 0}},
        }
    )
    hover = read_frame(server.stdout)
    contents = (hover or {}).get("result") or {}
    check(
        "hover returns markdown on a line with a finding",
        isinstance(contents, dict) and contents.get("contents", {}).get("kind") == "markdown",
    )

    # An unknown request must be answered, not ignored — a silent server leaves
    # the client blocked forever on a reply that never comes.
    send({"jsonrpc": "2.0", "id": 3, "method": "textDocument/codeLens", "params": {}})
    unknown = read_frame(server.stdout)
    check(
        "unknown requests get MethodNotFound",
        (unknown or {}).get("error", {}).get("code") == -32601,
    )

    send({"jsonrpc": "2.0", "id": 4, "method": "shutdown"})
    check("shutdown is acknowledged", (read_frame(server.stdout) or {}).get("id") == 4)

    send({"jsonrpc": "2.0", "method": "exit"})
    server.stdin.close()
    code = server.wait(timeout=30)
    check("server exits cleanly after exit", code == 0, f"exit code {code}")

    stderr = server.stderr.read().decode().strip()
    check("nothing unexpected on stderr", stderr == "", stderr[:120])


def measure_latency(binary: str) -> None:
    name, source = large_fixture()
    print(f"== latency: edit -> publishDiagnostics on {name} ({len(source)} bytes) ==")

    server = subprocess.Popen(
        [binary, "--stdio"], stdin=subprocess.PIPE, stdout=subprocess.PIPE
    )

    def send(payload: dict) -> None:
        server.stdin.write(frame(payload))
        server.stdin.flush()

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})
    read_frame(server.stdout)

    uri = "file:///large.rs"
    timings = []
    for round_index in range(EDIT_ROUNDS):
        body = f"{source}\n// edit {round_index}\n"
        started = time.perf_counter()
        send(did_open(uri, body) if round_index == 0 else did_change(uri, body, round_index + 1))
        response = read_frame(server.stdout)
        timings.append((time.perf_counter() - started) * 1000)
        if response is None or response.get("method") != "textDocument/publishDiagnostics":
            check(f"round {round_index} published diagnostics", False, str(response)[:120])
            break

    print("  per-edit latency (ms): " + ", ".join(f"{t:.1f}" for t in timings))
    worst = max(timings) if timings else float("inf")
    check(
        f"worst-case latency is inside the {LATENCY_BUDGET_MS}ms budget",
        worst < LATENCY_BUDGET_MS,
        f"worst {worst:.1f} ms",
    )

    send({"jsonrpc": "2.0", "method": "exit"})
    server.stdin.close()
    server.wait(timeout=30)


def check_usage(binary: str) -> None:
    print("== invocation ==")
    result = subprocess.run([binary], capture_output=True, timeout=30)
    # Without --stdio the process must not sit silently waiting on stdin.
    check("bare invocation exits non-zero", result.returncode != 0)
    check("bare invocation prints usage", b"USAGE" in result.stderr)

    version = subprocess.run([binary, "--version"], capture_output=True, timeout=30)
    check("--version succeeds", version.returncode == 0, version.stdout.decode().strip())


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2

    binary = sys.argv[1]
    if not os.path.exists(binary):
        print(f"error: no such binary: {binary}", file=sys.stderr)
        return 2

    check_usage(binary)
    run_session(binary)
    measure_latency(binary)

    print()
    if failures:
        print(f"FAILED: {len(failures)} check(s): " + ", ".join(failures))
        return 1
    print("All conformance checks passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
