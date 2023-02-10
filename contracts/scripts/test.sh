#!/bin/bash

set -euo pipefail

E2E_PATH=$(pwd)/e2e-tests
CONTRACTS_PATH=$(pwd)/contracts

# shellcheck source=contracts/scripts/test_env.sh
. "$CONTRACTS_PATH/scripts/test_env.sh"

pushd "$E2E_PATH"
cargo test button -- --test-threads 1 --nocapture
popd

exit $?
