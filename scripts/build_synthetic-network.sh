#!/bin/env bash

set -euo pipefail

source ./scripts/common.sh

GIT_COMMIT=${GIT_COMMIT:-72bbb4fde915e4132c19cd7ce3605364abac58a5}

TMPDIR=$(mktemp -d --tmpdir=.)
log "created a temporary folder at $TMPDIR"

pushd .

cd $TMPDIR
git clone https://github.com/daily-co/synthetic-network.git
cd synthetic-network
git checkout $GIT_COMMIT
sed -i 's/FROM node:12.20.2/FROM node:19.2/' Dockerfile

log "building base docker image for synthetic-network with support for synthetic-network"
docker build -t syntheticnet .

popd
log "removing temporary folder $TMPDIR"
rm -rf $TMPDIR

log "building docker image for aleph-node that supports synthetic-network"
docker build -t aleph-node:syntheticnet -f docker/Dockerfile.synthetic_network .

exit 0
