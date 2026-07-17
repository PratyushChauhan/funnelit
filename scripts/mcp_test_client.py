#!/usr/bin/env python3
"""Minimal Streamable HTTP MCP test client for Sumeru (stdlib only).

Exercises the Cursor-like lifecycle:
  initialize -> notifications/initialized -> GET SSE -> tools/list -> tools/call

Examples:
  SUMERU_TOKEN=... python3 scripts/mcp_test_client.py
  python3 scripts/mcp_test_client.py --keyring
  python3 scripts/mcp_test_client.py --keyring --clients 3 --call list_mcps
"""

from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from http.client import HTTPConnection, HTTPException, HTTPSConnection
from typing import Any
from urllib.parse import urlparse

DEFAULT_URL = "http://127.0.0.1:7341/mcp"
PROTOCOL = "2025-06-18"


def keyring_token() -> str:
    """Inputs: none. Outputs: endpoint bearer token from OS keyring."""
    out = subprocess.check_output(
        ["secret-tool", "lookup", "service", "sumeru", "username", "endpoint-token"],
        text=True,
    )
    return out.strip()


class McpClient:
    """Inputs: url, token, user_agent. Outputs: JSON-RPC helpers over Streamable HTTP."""

    def __init__(self, url: str, token: str, user_agent: str = "sumeru-mcp-test/0.1") -> None:
        self.url = url
        self.token = token
        self.user_agent = user_agent
        self.session_id: str | None = None

    def _headers(self, accept: str, content: bool = False) -> dict[str, str]:
        """Inputs: Accept value and whether a body is sent. Outputs: request headers."""
        h = {
            "Authorization": f"Bearer {self.token}",
            "Accept": accept,
            "User-Agent": self.user_agent,
            "MCP-Protocol-Version": PROTOCOL,
        }
        if content:
            h["Content-Type"] = "application/json"
        if self.session_id:
            h["Mcp-Session-Id"] = self.session_id
        return h

    def exchange(
        self,
        method: str,
        body: dict[str, Any] | None = None,
        *,
        accept: str = "application/json, text/event-stream",
        timeout: float = 10.0,
    ) -> tuple[int, dict[str, str], bytes, float]:
        """Inputs: HTTP method/body. Outputs: status, headers, body bytes, elapsed ms."""
        data = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request(
            self.url,
            data=data,
            headers=self._headers(accept, content=body is not None),
            method=method,
        )
        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                hdrs = {k.lower(): v for k, v in resp.headers.items()}
                raw = resp.read()
                status = resp.status
        except urllib.error.HTTPError as e:
            hdrs = {k.lower(): v for k, v in e.headers.items()}
            raw = e.read()
            status = e.code
        except urllib.error.URLError as e:
            hdrs = {}
            raw = str(e.reason if getattr(e, "reason", None) else e).encode()
            status = 0
        ms = (time.perf_counter() - t0) * 1000
        sid = hdrs.get("mcp-session-id")
        if sid:
            self.session_id = sid
        return status, hdrs, raw, ms

    def get_sse(self, *, peek: int = 64, timeout: float = 2.0) -> tuple[int, dict[str, str], bytes, float]:
        """Inputs: peek bytes. Outputs: status/headers/peek of keep-alive SSE without hanging."""
        u = urlparse(self.url)
        conn_cls = HTTPSConnection if u.scheme == "https" else HTTPConnection
        t0 = time.perf_counter()
        try:
            conn = conn_cls(
                u.hostname, u.port or (443 if u.scheme == "https" else 80), timeout=timeout
            )
        except Exception as e:
            return 0, {}, str(e).encode(), (time.perf_counter() - t0) * 1000
        try:
            conn.request(
                "GET",
                u.path or "/",
                headers=self._headers("text/event-stream"),
            )
            resp = conn.getresponse()
            hdrs = {k.lower(): v for k, v in resp.getheaders()}
            sid = hdrs.get("mcp-session-id")
            if sid:
                self.session_id = sid
            if conn.sock is not None:
                conn.sock.settimeout(0.8)
            raw = b""
            try:
                while len(raw) < peek:
                    chunk = resp.read(min(32, peek - len(raw)))
                    if not chunk:
                        break
                    raw += chunk
            except (TimeoutError, socket.timeout):
                pass
            return resp.status, hdrs, raw, (time.perf_counter() - t0) * 1000
        except (OSError, socket.timeout, TimeoutError, HTTPException) as e:
            return 0, {}, str(e).encode(), (time.perf_counter() - t0) * 1000
        finally:
            conn.close()

    def post_rpc(
        self, method: str, params: dict[str, Any] | None = None, *, id: int | None = 1
    ) -> tuple[int, Any, float]:
        """Inputs: JSON-RPC method/params. Outputs: status, parsed body or None, ms."""
        body: dict[str, Any] = {"jsonrpc": "2.0", "method": method}
        if id is not None:
            body["id"] = id
        if params is not None:
            body["params"] = params
        status, _, raw, ms = self.exchange("POST", body)
        if not raw:
            return status, None, ms
        try:
            return status, json.loads(raw.decode()), ms
        except json.JSONDecodeError:
            return status, raw.decode("utf-8", "replace"), ms


def run_lifecycle(
    url: str,
    token: str,
    name: str,
    *,
    call: str | None,
    call_args: dict[str, Any],
    get_read: int,
) -> dict[str, Any]:
    """Inputs: client identity + optional tools/call. Outputs: step results dict."""
    c = McpClient(url, token, user_agent=f"sumeru-mcp-test/{name}")
    steps: list[dict[str, Any]] = []
    ok = True

    def step(label: str, good: bool, **extra: Any) -> None:
        nonlocal ok
        ok = ok and good
        steps.append({"step": label, "ok": good, **extra})

    st, body, ms = c.post_rpc(
        "initialize",
        {
            "protocolVersion": PROTOCOL,
            "capabilities": {},
            "clientInfo": {"name": name, "version": "0.1"},
        },
        id=1,
    )
    step(
        "initialize",
        st == 200 and isinstance(body, dict) and "result" in body,
        status=st,
        ms=round(ms, 1),
        session=c.session_id,
        server=(body or {}).get("result", {}).get("serverInfo") if isinstance(body, dict) else None,
    )

    st, _, ms = c.post_rpc("notifications/initialized", id=None)
    step("initialized", st in (200, 202), status=st, ms=round(ms, 1))

    st, hdrs, raw, ms = c.get_sse(peek=get_read)
    ct = hdrs.get("content-type", "")
    is_sse = ct.split(";", 1)[0].strip().lower() == "text/event-stream"
    step(
        "GET_sse",
        st == 200 and is_sse,
        status=st,
        ms=round(ms, 1),
        content_type=ct,
        preview=raw[:40].decode("utf-8", "replace"),
    )

    st, body, ms = c.post_rpc("tools/list", id=2)
    tools = []
    if isinstance(body, dict):
        tools = [t.get("name") for t in body.get("result", {}).get("tools", [])]
    step(
        "tools/list",
        st == 200 and len(tools) >= 1,
        status=st,
        ms=round(ms, 1),
        tools=tools,
    )

    if call:
        st, body, ms = c.post_rpc(
            "tools/call",
            {"name": call, "arguments": call_args},
            id=3,
        )
        is_err = isinstance(body, dict) and body.get("result", {}).get("isError") is True
        sc = None
        if isinstance(body, dict):
            sc = body.get("result", {}).get("structuredContent")
        step(
            f"tools/call:{call}",
            st == 200 and not is_err and isinstance(sc, dict),
            status=st,
            ms=round(ms, 1),
            structured_type=type(sc).__name__,
            structured_keys=list(sc)[:8] if isinstance(sc, dict) else None,
        )

    return {"client": name, "ok": ok, "steps": steps}


def main() -> int:
    """Inputs: CLI argv/env. Outputs: process exit code (0 on pass)."""
    p = argparse.ArgumentParser(description="Sumeru Streamable HTTP MCP test client")
    p.add_argument("--url", default=os.environ.get("SUMERU_URL", DEFAULT_URL))
    p.add_argument("--token", default=os.environ.get("SUMERU_TOKEN"))
    p.add_argument("--keyring", action="store_true", help="Load token via secret-tool")
    p.add_argument("--clients", type=int, default=1, help="Concurrent clients")
    p.add_argument("--call", default="list_mcps", help="tools/call name (empty to skip)")
    p.add_argument(
        "--args",
        default="{}",
        help='JSON object for tools/call arguments (default "{}")',
    )
    p.add_argument("--get-read", type=int, default=64, help="Bytes to read from GET SSE")
    p.add_argument("--json", action="store_true", help="Print full JSON report")
    args = p.parse_args()

    token = args.token
    if args.keyring or not token:
        try:
            token = keyring_token()
        except Exception as e:
            print(f"error: no token (set SUMERU_TOKEN, --token, or --keyring): {e}", file=sys.stderr)
            return 2
    if not token:
        print("error: empty token", file=sys.stderr)
        return 2

    try:
        call_args = json.loads(args.args)
    except json.JSONDecodeError as e:
        print(f"error: --args must be JSON object: {e}", file=sys.stderr)
        return 2
    if not isinstance(call_args, dict):
        print("error: --args must be a JSON object", file=sys.stderr)
        return 2

    call = args.call or None
    n = max(1, args.clients)
    reports: list[dict[str, Any]] = []
    lock = threading.Lock()

    def one(i: int) -> dict[str, Any]:
        r = run_lifecycle(
            args.url,
            token,
            f"c{i}",
            call=call,
            call_args=call_args,
            get_read=args.get_read,
        )
        with lock:
            reports.append(r)
        return r

    t0 = time.perf_counter()
    if n == 1:
        one(1)
    else:
        with ThreadPoolExecutor(max_workers=n) as ex:
            futs = [ex.submit(one, i) for i in range(1, n + 1)]
            for f in as_completed(futs):
                f.result()
    elapsed = (time.perf_counter() - t0) * 1000
    reports.sort(key=lambda r: r["client"])

    all_ok = all(r["ok"] for r in reports)
    if args.json:
        print(json.dumps({"ok": all_ok, "elapsed_ms": round(elapsed, 1), "clients": reports}, indent=2))
    else:
        for r in reports:
            mark = "PASS" if r["ok"] else "FAIL"
            print(f"[{mark}] {r['client']}")
            for s in r["steps"]:
                sm = "ok" if s["ok"] else "FAIL"
                extra = {k: v for k, v in s.items() if k not in ("step", "ok")}
                print(f"  {sm:4} {s['step']:20} {extra}")
        print(f"{'PASS' if all_ok else 'FAIL'} — {n} client(s) in {elapsed:.0f}ms")
    return 0 if all_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
