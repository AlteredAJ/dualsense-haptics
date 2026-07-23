#!/bin/bash
# Beta-key admin CLI for the DualSense Haptics license worker.
#
# Beta keys activate the app WITHOUT a Gumroad purchase, so you can hand testers
# a free key instead of comping orders one by one.
#
# Setup (once):
#   export ADMIN_KEY="<the same value you set with: wrangler secret put ADMIN_KEY>"
#
# Usage:
#   ./scripts/beta.sh mint [count] [note]     # generate N beta keys (default 1)
#   ./scripts/beta.sh list                    # list all minted beta keys
#   ./scripts/beta.sh revoke <KEY>            # disable a key
#   ./scripts/beta.sh unrevoke <KEY>          # re-enable a key
#
# Examples:
#   ./scripts/beta.sh mint 10 "discord wave 1"
#   ./scripts/beta.sh revoke BETA-AB12-CD34-EF56

set -euo pipefail

BASE="${LICENSE_SERVER:-https://dualsense-haptics-license.universal-dualsense-haptics.workers.dev}"

if [ -z "${ADMIN_KEY:-}" ]; then
  echo "error: ADMIN_KEY is not set. Run: export ADMIN_KEY=..." >&2
  exit 1
fi

cmd="${1:-}"
case "$cmd" in
  mint)
    count="${2:-1}"
    note="${3:-}"
    curl -s -X POST "$BASE/admin/mint-beta" \
      -H 'Content-Type: application/json' \
      -H "X-Admin-Key: $ADMIN_KEY" \
      -d "{\"count\": $count, \"note\": \"$note\"}"
    echo
    ;;
  list)
    curl -s "$BASE/admin/list-beta" -H "X-Admin-Key: $ADMIN_KEY"
    echo
    ;;
  revoke)
    key="${2:?usage: beta.sh revoke <KEY>}"
    curl -s -X POST "$BASE/admin/revoke" \
      -H 'Content-Type: application/json' \
      -H "X-Admin-Key: $ADMIN_KEY" \
      -d "{\"key\": \"$key\", \"revoked\": true}"
    echo
    ;;
  unrevoke)
    key="${2:?usage: beta.sh unrevoke <KEY>}"
    curl -s -X POST "$BASE/admin/revoke" \
      -H 'Content-Type: application/json' \
      -H "X-Admin-Key: $ADMIN_KEY" \
      -d "{\"key\": \"$key\", \"revoked\": false}"
    echo
    ;;
  *)
    echo "usage: beta.sh {mint [count] [note] | list | revoke <KEY> | unrevoke <KEY>}" >&2
    exit 1
    ;;
esac
