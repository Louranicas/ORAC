#!/bin/bash
# ORAC hook forwarder - routes Claude Code hook events to ORAC sidecar HTTP server.
# Usage: orac-hook.sh <EventName> [timeout_seconds]
# Reads hook event JSON from stdin, POSTs to http://localhost:8133/hooks/<EventName>,
# outputs response JSON to stdout.
# MUST always exit 0 - never block Claude Code.

EVENT="${1:-unknown}"
TIMEOUT="${2:-5}"
ORAC_URL="${ORAC_URL:-http://localhost:8133}"

# Read hook event from stdin (Claude Code pipes the event payload)
BODY="$(cat 2>/dev/null)" || true
[ -z "$BODY" ] && BODY='{}'

# Forward to ORAC endpoint, output response for Claude Code to consume
curl -s --max-time "$TIMEOUT" -X POST "${ORAC_URL}/hooks/${EVENT}" \
    -H "Content-Type: application/json" \
    -d "$BODY" 2>/dev/null || true

exit 0
