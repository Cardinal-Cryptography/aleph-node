#!/usr/bin/env bash
set -euo pipefail

if [[ $STORAGE_DEBUG == yes ]]; then
  aleph-e2e-client --node "$NODE_URL" --storage-debug
else
  aleph-e2e-client --node "$NODE_URL"
fi

echo "Done!"
