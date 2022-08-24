#!/usr/bin/env bash

set -euo pipefail

TEST_CASES=""
MIN_VALIDATOR_COUNT=""
RESERVED_SEATS=""
NON_RESERVED_SEATS=""

while [[ $# -gt 0 ]]; do
  case $1 in
  -h|--help)
    usage()
    exit 0
    shift 2
    ;;
  -t|--test-cases)
    export TEST_CASES="$2"
    shift 2
    ;;
  -m|--min-validator-count)
    export MIN_VALIDATOR_COUNT="$2"
    shift 2
    ;;
  -r|--reserved-seats)
    export RESERVED_SEATS="$2"
    shift 2
    ;;
  -n|--non-reserved-seats)
    export NON_RESERVED_SEATS="$2"
    shift 2
    ;;
  *)
    echo "Unrecognized argument $1!"
    exit 1
    ;;
  esac
done

# source docker/env

docker run -v $(pwd)/docker/data:/data --network container:Node0 -e TEST_CASES -e MIN_VALIDATOR_COUNT \
  -e RESERVED_SEATS -e NON_RESERVED_SEATS -e NODE_URL=127.0.0.1:9943 -e RUST_LOG=info aleph-e2e-client:latest

function usage() {
    cat << EOF
  Usage:
    $0
      --test-cases
        test cases to run
      --min-validator-count
        minimum number of validators for which the chain works as expected
      --reserved-seats
        number of reserved seats available to validators
      --non-reserved-seats
        number of non-reserved seats available to validators
  EOF
    exit 0
}

exit $?
