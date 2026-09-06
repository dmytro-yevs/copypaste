#!/usr/bin/env python3
"""A STUB of the Supabase surface CopyPaste uses. NOT a Supabase deployment.

Answers the four GoTrue calls and the three PostgREST calls `copypaste-cloud`
makes, keeps rows in memory, and dumps them to --dump on each write so the demo
can inspect exactly what left the device.

It has no row-level security, no JWT verification and no Postgres, and it is
permissive where the real service is strict. A passing run means the client is
wired end to end, never that this works against Supabase.
"""

import argparse
import json
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

USER_ID = "00000000-0000-4000-8000-00000000da7a"

STATE_LOCK = threading.Lock()
ROWS = {}  # item_id -> row dict
ARGS = None


def now_ms():
    return int(time.time() * 1000)


def dump_rows():
    if not ARGS.dump:
        return
    with open(ARGS.dump, "w") as f:
        json.dump(sorted(ROWS.values(), key=lambda r: r["item_id"]), f, indent=2)


def _safe_path(raw):
    path = urlparse(raw or "").path or "/"
    path = re.sub(r"(?i)/Users/[^/]+", "/Users/<redacted>", path)
    path = re.sub(r"(?i)/home/[^/]+", "/home/<redacted>", path)
    return path


def _is_realtime(path):
    return path == "/realtime/v1/websocket" or path.startswith("/realtime/v1/")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_request(self, code="-", size="-"):
        status = getattr(code, "value", code)
        self.log_message("%s %s %s", self.command or "-", _safe_path(self.path), status)

    def log_message(self, fmt, *args):
        if ARGS is None or not ARGS.verbose:
            return
        sys.stderr.write("%s\n" % (fmt % args))

    # -- plumbing ----------------------------------------------------------
    def _body(self):
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b""
        return json.loads(raw) if raw else None

    def _reply(self, status, payload=None):
        body = b"" if payload is None else json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        if not body:
            # Run 34016710899: empty 201/realtime 404 without a flushed
            # close-delimited length left reqwest waiting for EOF.
            self.send_header("Connection", "close")
            self.close_connection = True
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def _authorized(self):
        # Deliberately shallow: the point of the stub is the request shapes, not
        # authentication. A real deployment verifies the JWT and applies RLS.
        auth = self.headers.get("Authorization", "")
        return auth.startswith("Bearer ") and len(auth) > len("Bearer ")

    # -- routes ------------------------------------------------------------
    def do_POST(self):
        url = urlparse(self.path)
        if _is_realtime(url.path):
            self._body()
            return self._reply(404)
        query = parse_qs(url.query)

        if url.path == "/auth/v1/token":
            grant = (query.get("grant_type") or [""])[0]
            body = self._body() or {}
            if grant == "password" and body.get("password") != ARGS.password:
                return self._reply(400, {"error": "invalid_grant"})
            return self._reply(200, {
                "access_token": f"stub-access-{now_ms()}",
                # Rotated on every call, exactly as GoTrue does, so a client
                # that fails to persist the new one breaks here too.
                "refresh_token": f"stub-refresh-{now_ms()}",
                "expires_in": 3600,
                "user": {"id": USER_ID},
            })

        if url.path == "/auth/v1/logout":
            return self._reply(204)

        if url.path == "/rest/v1/clipboard_items":
            if not self._authorized():
                return self._reply(401, {"message": "no bearer"})
            incoming = self._body() or []
            with STATE_LOCK:
                for row in incoming:
                    ROWS[row["item_id"]] = row
                dump_rows()
            return self._reply(201)

        self._reply(404, {"message": "no such stub route"})

    def do_PATCH(self):
        url = urlparse(self.path)
        if url.path != "/rest/v1/clipboard_items":
            return self._reply(404, {"message": "no such stub route"})
        if not self._authorized():
            return self._reply(401, {"message": "no bearer"})

        query = parse_qs(url.query)
        patch = self._body() or {}
        ids = []
        raw = (query.get("item_id") or [""])[0]
        match = re.fullmatch(r"in\.\((.*)\)", raw)
        if match:
            ids = [i for i in match.group(1).split(",") if i]

        with STATE_LOCK:
            for item_id in ids:
                row = ROWS.get(item_id)
                if row is None:
                    # A tombstone for a row this backend never saw still has to
                    # exist, or the delete cannot reach another device.
                    row = {
                        "item_id": item_id,
                        "content_type": "text",
                        "origin_device_id": "unknown",
                    }
                    ROWS[item_id] = row
                row.update(patch)
                row["ciphertext"] = ""
                row["nonce"] = ""
                row["deleted"] = True
            dump_rows()
        self._reply(204)

    def do_GET(self):
        url = urlparse(self.path)
        if _is_realtime(url.path):
            return self._reply(404)
        if url.path != "/rest/v1/clipboard_items":
            return self._reply(404, {"message": "no such stub route"})
        if not self._authorized():
            return self._reply(401, {"message": "no bearer"})

        query = parse_qs(url.query)
        after = None
        since = 0

        keyset = (query.get("or") or [""])[0]
        if keyset:
            # The compound cursor: strictly after the pair (created_at, item_id).
            # Exclusive is correct *only* in this form, because the pair is a
            # total order with no ties (manifest 05 §5.1 row 6, INV-N1).
            match = re.fullmatch(
                r"\(created_at\.gt\.(\d+),and\(created_at\.eq\.(\d+),item_id\.gt\.([A-Za-z0-9_-]+)\)\)",
                keyset,
            )
            if not match or match.group(1) != match.group(2):
                return self._reply(400, {"message": "unparseable keyset cursor"})
            since = int(match.group(1))
            after = match.group(3)
        else:
            raw = (query.get("created_at") or ["gte.0"])[0]
            if raw.startswith("gte."):
                since = int(raw[4:])
            elif raw.startswith("gt."):
                # A strict bound on the millisecond *alone* drops every row
                # sharing the boundary millisecond (manifest 05 §4.4).
                return self._reply(400, {"message": "exclusive cursor bound"})

        order = (query.get("order") or [""])[0]
        if order != "created_at.asc,item_id.asc":
            # Equally load-bearing: a forward cursor cannot drain a
            # newest-first page, and the keyset's tie-break needs item_id in
            # the same direction.
            return self._reply(400, {"message": "page order is not the keyset order"})
        limit = int((query.get("limit") or ["100"])[0])

        def key(row):
            return (int(row.get("created_at", 0)), row["item_id"])

        with STATE_LOCK:
            if after is None:
                rows = [r for r in ROWS.values() if key(r)[0] >= since]
            else:
                rows = [r for r in ROWS.values() if key(r) > (since, after)]
        rows.sort(key=key)
        self._reply(200, rows[:limit])


def main():
    global ARGS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=47800)
    parser.add_argument("--password", default="stub-password")
    parser.add_argument("--dump", help="write every stored row here on each write")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    ARGS = parser.parse_args()
    if ARGS.self_test:
        raise SystemExit(self_test())

    server = ThreadingHTTPServer(("127.0.0.1", ARGS.port), Handler)
    print(f"stub backend (NOT Supabase) listening on 127.0.0.1:{ARGS.port}", flush=True)
    server.serve_forever()


def _start_stub(verbose=False, password="stub-password"):
    global ARGS
    ARGS = argparse.Namespace(port=0, password=password, dump=None, verbose=verbose)
    with STATE_LOCK:
        ROWS.clear()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def _http(port):
    import http.client

    last = None
    for _ in range(50):
        conn = http.client.HTTPConnection("127.0.0.1", port, timeout=2)
        try:
            conn.connect()
            return conn
        except OSError as err:
            last = err
            time.sleep(0.01)
    raise last


def _exchange(conn, method, url, body=None, headers=None):
    hdrs = {"Connection": "keep-alive"}
    if headers:
        hdrs.update(headers)
    started = time.monotonic()
    conn.request(method, url, body=body, headers=hdrs)
    resp = conn.getresponse()
    payload = resp.read()
    return resp.status, payload, resp.getheader("Content-Length"), time.monotonic() - started


def self_test():
    import contextlib
    import io

    passed = 0
    failed = 0

    def check(name, ok):
        nonlocal passed, failed
        if ok:
            passed += 1
            print(f"  ok    {name}")
        else:
            failed += 1
            print(f"  FAIL  {name}")

    server = _start_stub(verbose=True)
    port = server.server_address[1]
    conn = _http(port)
    auth = {"Authorization": "Bearer stub-access", "Content-Type": "application/json"}
    row = {
        "item_id": "it-1",
        "ciphertext": "SECRET_CIPHER",
        "nonce": "AA==",
        "content_type": "text",
        "created_at": 1,
        "deleted": False,
        "origin_device_id": "stub",
        "signature": "",
    }
    logs = io.StringIO()
    with contextlib.redirect_stderr(logs):
        status, body, length, elapsed = _exchange(
            conn,
            "POST",
            "/rest/v1/clipboard_items?apikey=secret-key&password=hunter2",
            body=json.dumps([row]),
            headers=auth,
        )
        check("POST empty 201 completes promptly",
              status == 201 and body == b"" and length == "0" and elapsed < 1.0)
        status, body, length, elapsed = _exchange(
            conn,
            "GET",
            "/rest/v1/clipboard_items?order=created_at.asc,item_id.asc&created_at=gte.0",
            headers={"Authorization": "Bearer stub-access"},
        )
        pulled = json.loads(body) if body else []
        check("keep-alive GET after empty 201 returns the upsert",
              status == 200 and elapsed < 1.0 and pulled == [row])
        status, body, length, elapsed = _exchange(
            conn,
            "GET",
            "/realtime/v1/websocket?apikey=secret-key&vsn=2.0.0",
        )
        check("realtime refusal completes promptly",
              status == 404 and body == b"" and length == "0" and elapsed < 1.0)
        status, body, _, _ = _exchange(
            conn,
            "POST",
            "/auth/v1/token?grant_type=password",
            body=json.dumps({"email": "native@example.test", "password": "stub-password"}),
            headers={"Content-Type": "application/json"},
        )
        token = json.loads(body) if body else {}
        check("password grant still returns JSON tokens",
              status == 200 and token.get("user", {}).get("id") == USER_ID
              and token.get("access_token", "").startswith("stub-access-"))
        status, body, _, _ = _exchange(
            conn,
            "POST",
            "/auth/v1/token?grant_type=password",
            body=json.dumps({"email": "native@example.test", "password": "wrong"}),
            headers={"Content-Type": "application/json"},
        )
        check("invalid password still returns JSON invalid_grant",
              status == 400 and json.loads(body) == {"error": "invalid_grant"})
        status, body, _, _ = _exchange(conn, "GET", "/rest/v1/clipboard_items")
        check("missing bearer still returns JSON 401",
              status == 401 and json.loads(body) == {"message": "no bearer"})
        _exchange(conn, "GET", "/Users/dmytro/Library/secrets?apikey=secret-key")
    recorded = logs.getvalue()
    secret_hits = [
        token for token in (
            "secret-key", "hunter2", "SECRET_CIPHER", "stub-access",
            "stub-password", "apikey=", "password=", "ciphertext",
            "dmytro", "/Users/dmytro",
        )
        if token in recorded
    ]
    check("verbose logs method, sanitized path, and status",
          "POST /rest/v1/clipboard_items 201" in recorded
          and "GET /rest/v1/clipboard_items 200" in recorded
          and "GET /realtime/v1/websocket 404" in recorded
          and "POST /auth/v1/token 200" in recorded)
    check("verbose logs redact secrets, query, body, and user paths",
          not secret_hits and "GET /Users/<redacted>/Library/secrets 404" in recorded)
    conn.close()
    server.shutdown()
    server.server_close()
    print(f"{passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    main()
