#!/usr/bin/env bash

set -euo pipefail

# This is required by Substrate: MinValidatorCount in pallet_Staking.
MIN_VALIDATOR_COUNT=4
# This is arbitrary.
MAX_VALIDATOR_COUNT=20

function set_randomized_test_params {
  VALIDATOR_COUNT=$(shuf -i "${MIN_VALIDATOR_COUNT}"-"${MAX_VALIDATOR_COUNT}" -n 1)
  # Assumes there is at least one reserved seat for validators.
  RESERVED_SEATS=$(shuf -i 1-"${VALIDATOR_COUNT}" -n 1)
  NON_RESERVED_SEATS=$((${VALIDATOR_COUNT} - ${RESERVED_SEATS}))
}

function usage {
    cat << EOF
Usage:
  $0
    -t
      test cases to run
    -r
      whether to randomize test case params, "true" and "false" values supported
      if randomization is performed, the `--reserved-seats` and `--non-reserved-seats` params are ignored
    -f
      number of reserved seats available to validators; ignored if empty or `--non-reserved-seats` is empty
    -n
      number of non-reserved seats available to validators; ignored if empty or `--reserved-seats` is empty
EOF
  exit 0
}

while getopts "h:t:r:f:n:" flag
do
  case "${flag}" in
    h) usage;;
    t) TEST_CASES="${OPTARG}";;
    r) RANDOMIZED="${OPTARG}";;
    f) RESERVED_SEATS="${OPTARG}";;
    n) NON_RESERVED_SEATS="${OPTARG}";;
    *)
      echo "Unrecognized argument "${flag}"!"
      usage
      exit 1
      ;;
  esac
done

ARGS=(
  --network "container:Node0"
  -e NODE_URL="127.0.0.1:9943"
  -e RUST_LOG=info
)

if [[ -n "${TEST_CASES:-}" ]]; then
  ARGS+=(-e TEST_CASES="${TEST_CASES}")
fi

RANDOMIZED="${RANDOMIZED:-"false"}"

# If randomization requested, generate random test params, ignore test params if provided.
if [[ "${RANDOMIZED}" == "true" ]]; then
  set_randomized_test_params
  echo "Test case params have been randomized."
elif [[ "${RANDOMIZED}" == "false" ]]; then
  echo "Test case params have not been randomized."
else
  echo "Only 'true' and 'false' values supported, "${RANDOMIZED}" provided!"
  exit 1
fi

# If both test params are not empty, pass them. Otherwise, do not pass them.
if [[ -n "${RESERVED_SEATS:-}" && -n "${NON_RESERVED_SEATS:-}" ]]; then
  echo "Test case params: "${RESERVED_SEATS}" reserved and "${NON_RESERVED_SEATS}" non-reserved seats."
  ARGS+=(
    -e "${RESERVED_SEATS}"
    -e "${NON_RESERVED_SEATS}"
  )
else
  echo "Falling back on default test case param values."
fi

docker run -v $(pwd)/docker/data:/data "${ARGS[@]}" aleph-e2e-client:latest

exit $?
