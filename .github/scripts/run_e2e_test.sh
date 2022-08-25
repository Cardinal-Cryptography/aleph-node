#!/usr/bin/env bash

set -euo pipefail

TEST_CASES=""
RANDOMIZED="false"

# This is required by the `Staking` pallet from Substrate.
MIN_VALIDATOR_COUNT=4
MAX_VALIDATOR_COUNT=""
VALIDATOR_COUNT=""
RESERVED_SEATS=""
NON_RESERVED_SEATS=""

while [[ $# -gt 0 ]]; do
  case $1 in
  -h|--help)
    usage
    exit 0
    shift 2
    ;;
  -t|--test-cases)
    TEST_CASES="$2"
    shift 2
    ;;
  -r|--randomized)
    RANDOMIZED="$2"
    shift 2
    ;;
  *)
    echo "Unrecognized argument $1!"
    exit 1
    ;;
  esac
done

if [[ "${RANDOMIZED}" == "true" ]]; then
  set_randomized_test_params
fi

# source docker/env

docker run -v $(pwd)/docker/data:/data --network container:Node0 -e TEST_CASES="${TEST_CASES}" \
  -e MIN_VALIDATOR_COUNT="${MIN_VALIDATOR_COUNT}" -e RESERVED_SEATS="${RESERVED_SEATS}" \
  -e NON_RESERVED_SEATS="${NON_RESERVED_SEATS}" -e NODE_URL=127.0.0.1:9943 -e RUST_LOG=info aleph-e2e-client:latest

function set_randomized_test_params() {
  # This is arbitrary.
  MAX_VALIDATOR_COUNT=20
  VALIDATOR_COUNT=$(shuf -i "${MIN_VALIDATOR_COUNT}"-"${MAX_VALIDATOR_COUNT}" -n 1)
  # Assumes there is at least one reserved seat for validators.
  RESERVED_SEATS=$(shuf -i 1-"${VALIDATOR_COUNT}" -n 1)
  NON_RESERVED_SEATS=$((${VALIDATOR_COUNT} - ${RESERVED_SEATS}))
}

function usage {
    cat << EOF
  Usage:
    $0
      --test-cases
        test cases to run
      --randomized
        whether to randomize test case params
  EOF
    exit 0
}

exit $?
