#!/usr/bin/env bash
set -euo pipefail

aleph-e2e-client --node "$NODE_URL" "${STORAGE_DEBUG-}"

echo "Done!"
