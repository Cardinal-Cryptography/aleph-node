#!/usr/bin/env bash

set -euo pipefail

TEST_CASES=""
RANDOMIZED="false"
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
  -f|--reserved-seats)
    RESERVED_SEATS="$2"
    shift 2
    ;;
  -n|--non-reserved-seats)
    NON_RESERVED_SEATS="$2"
    shift 2
    ;;
  *)
    echo "Unrecognized argument $1!"
    exit 1
    ;;
  esac
done

# If randomization requested, generate random test params. Otherwise: a) in case of non-empty params, pass them,
# b) in case of empty params, do not pass them.
if [[ "${RANDOMIZED}" == "true" ]]; then
  set_randomized_test_params
  echo "Using randomized test case params: ${RESERVED_SEATS} reserved and ${NON_RESERVED_SEATS} non-reserved seats."
  run_docker_with_test_case_params
elif [[ "${RANDOMIZED}" == "false" ]]; then
  if [[ -n "${RESERVED_SEATS}" && -n "${NON_RESERVED_SEATS}" ]]; then
    echo "Using provided test case params: ${RESERVED_SEATS} reserved and ${NON_RESERVED_SEATS} non-reserved seats."
    run_docker_with_test_case_params
  else
    echo "Falling back on default test case param values."
    run_docker_without_test_case_params
  fi
else
  echo "Only 'true' and 'false' values supported, ${RANDOMIZED} provided!"
  exit 1
fi

function set_randomized_test_params {
  # This is arbitrary.
  MAX_VALIDATOR_COUNT=20
  VALIDATOR_COUNT=$(shuf -i "${MIN_VALIDATOR_COUNT}"-"${MAX_VALIDATOR_COUNT}" -n 1)
  # Assumes there is at least one reserved seat for validators.
  RESERVED_SEATS=$(shuf -i 1-"${VALIDATOR_COUNT}" -n 1)
  NON_RESERVED_SEATS=$((${VALIDATOR_COUNT} - ${RESERVED_SEATS}))
}

function run_docker_with_test_case_params {
  docker run -v $(pwd)/docker/data:/data --network container:Node0 -e TEST_CASES="${TEST_CASES}" \
    -e RESERVED_SEATS="${RESERVED_SEATS}" -e NON_RESERVED_SEATS="${NON_RESERVED_SEATS}" -e NODE_URL=127.0.0.1:9943 \
    -e RUST_LOG=info aleph-e2e-client:latest
}

function run_docker_without_test_case_params {
  docker run -v $(pwd)/docker/data:/data --network container:Node0 -e TEST_CASES="${TEST_CASES}" \
    -e NODE_URL=127.0.0.1:9943 -e RUST_LOG=info aleph-e2e-client:latest
}

function usage {
    cat << EOF
  Usage:
    $0
      --test-cases
        test cases to run
      --randomized
        whether to randomize test case params, "true" and "false" values supported
        if randomization is performed, the `--reserved-seats` and `non-reserved-seats` params are ignored
      --reserved-seats
        number of reserved seats available to validators
      --non-reserved-seats
        number of non-reserved seats available to validators
  EOF
    exit 0
}

exit $?
