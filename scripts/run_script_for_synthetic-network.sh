#!/bin/env bash

set -euo pipefail

source ./scripts/common.sh

GIT_COMMIT=${GIT_COMMIT:-72bbb4fde915e4132c19cd7ce3605364abac58a5}
SCRIPT_PATH=${SCRIPT_PATH:-scripts/vendor/synthetic-network/frontend/udp_rate_sine_demo.js}
SCRIPT_PATH=$(realpath $SCRIPT_PATH)

TMPDIR="$(dirname $0)/vendor"
mkdir -p $TMPDIR
log "created a temporary folder at $TMPDIR"

log "cloning synthetic-network's git repo"
cd $TMPDIR
if [[ ! -d ./synthetic-network ]]; then
    git clone https://github.com/daily-co/synthetic-network.git
fi
cd synthetic-network
git fetch origin
git checkout $GIT_COMMIT
cd frontend

log "running .js script"
cp $SCRIPT_PATH ./script.js
node script.js

exit 0
