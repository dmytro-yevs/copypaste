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


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        if ARGS.verbose:
            super().log_message(fmt, *args)

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
        self.end_headers()
        if body:
            self.wfile.write(body)

    def _authorized(self):
        # Deliberately shallow: the point of the stub is the request shapes, not
        # authentication. A real deployment verifies the JWT and applies RLS.
        auth = self.headers.get("Authorization", "")
        return auth.startswith("Bearer ") and len(auth) > len("Bearer ")

    # -- routes ------------------------------------------------------------
    def do_POST(self):
        url = urlparse(self.path)
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
        if url.path != "/rest/v1/clipboard_items":
            return self._reply(404, {"message": "no such stub route"})
        if not self._authorized():
            return self._reply(401, {"message": "no bearer"})

        query = parse_qs(url.query)
        since = 0
        raw = (query.get("created_at") or ["gte.0"])[0]
        if raw.startswith("gte."):
            since = int(raw[4:])
        elif raw.startswith("gt."):
            # The client must never send this: a strict bound drops every row
            # sharing the boundary millisecond (manifest 05 §4.4).
            return self._reply(400, {"message": "exclusive cursor bound"})
        order = (query.get("order") or [""])[0]
        if not order.startswith("created_at.asc"):
            # Equally load-bearing: a forward cursor cannot drain a
            # newest-first page.
            return self._reply(400, {"message": "page order is not ascending"})
        limit = int((query.get("limit") or ["100"])[0])

        with STATE_LOCK:
            rows = [r for r in ROWS.values() if int(r.get("created_at", 0)) >= since]
        rows.sort(key=lambda r: (int(r.get("created_at", 0)), r["item_id"]))
        self._reply(200, rows[:limit])


def main():
    global ARGS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=47800)
    parser.add_argument("--password", default="stub-password")
    parser.add_argument("--dump", help="write every stored row here on each write")
    parser.add_argument("--verbose", action="store_true")
    ARGS = parser.parse_args()

    server = ThreadingHTTPServer(("127.0.0.1", ARGS.port), Handler)
    print(f"stub backend (NOT Supabase) listening on 127.0.0.1:{ARGS.port}", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
