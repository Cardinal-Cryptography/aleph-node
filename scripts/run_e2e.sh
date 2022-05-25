#!/bin/bash

set -e

# change it when increasing number of node containers
export NODE_COUNT=5

cd e2e-tests/

RUST_LOG=aleph_e2e_client=info,aleph-client=info cargo run -- --node 127.0.0.1:9943 --validators_count $NODE_COUNT

exit $?
