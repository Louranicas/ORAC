#!/usr/bin/env python3
"""Minimal HTTP hook test server for validating Claude Code HTTP hook format.

Usage:
    python3 scripts/test-hook-server.py

Then add ONE test hook to ~/.claude/settings.json alongside existing bash hooks:
    "SessionStart": [
        { existing bash hook... },
        { "type": "http", "url": "http://localhost:8133/hooks/SessionStart", "timeout": 5000 }
    ]

Trigger a new session, verify this server receives the POST, then remove the test hook.
This de-risks the hook migration before building the full ORAC hook server.
"""

import json
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from datetime import datetime


class HookHandler(BaseHTTPRequestHandler):
    """Log all incoming hook requests with full body."""

    def do_POST(self):
        length = int(self.headers.get("content-length", 0))
        body = self.rfile.read(length) if length > 0 else b""

        timestamp = datetime.now().strftime("%H:%M:%S.%f")[:-3]
        path = self.path

        try:
            parsed = json.loads(body) if body else {}
            pretty = json.dumps(parsed, indent=2)[:500]
        except json.JSONDecodeError:
            pretty = body.decode("utf-8", errors="replace")[:500]

        print(f"\n[{timestamp}] POST {path}")
        print(f"  Content-Type: {self.headers.get('content-type', 'none')}")
        print(f"  Body ({length} bytes):")
        for line in pretty.split("\n"):
            print(f"    {line}")

        # Return 200 with empty result (Claude Code expects this)
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"result":"ok"}')

    def log_message(self, format, *args):
        """Suppress default access log — we print our own."""
        pass


def main():
    port = 8133
    server = HTTPServer(("127.0.0.1", port), HookHandler)
    print(f"ORAC test hook server listening on http://127.0.0.1:{port}")
    print("Waiting for Claude Code HTTP hook requests...")
    print("Press Ctrl+C to stop.\n")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down.")
        server.server_close()


if __name__ == "__main__":
    main()
