#!/bin/bash

set -e

cd e2e-tests/

RUST_LOG=aleph_e2e_client=info cargo +nightly run -- --node 127.0.0.1:9943

exit $?
