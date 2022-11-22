#!/bin/bash

set -e

cd e2e-tests/

E2E_CONFIG="--node 127.0.0.1:9943" RUST_LOG=aleph_e2e_client=info,aleph-client=info cargo test -- --nocapture

exit $?
