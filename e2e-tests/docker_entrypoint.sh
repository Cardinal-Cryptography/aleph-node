#!/usr/bin/env bash
set -euo pipefail

aleph-e2e-client --node "$NODE_URL" "$DEBUG_STORAGE"

echo "Done!"
