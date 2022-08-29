#!/usr/bin/env bash
set -euo pipefail

RESERVED_SEATS=${RESERVED_SEATS:-}
NON_RESERVED_SEATS=${NON_RESERVED_SEATS:-}

ARGS="--node "${NODE_URL}" --test-cases "${TEST_CASES}""

# If test case params are both not empty, run client with them. Otherwise, run without params.
if [[ -n "${RESERVED_SEATS}" && -n "${NON_RESERVED_SEATS}" ]]; then
  ARGS="${ARGS} --reserved-seats "${RESERVED_SEATS}" --non-reserved-seats "${NON_RESERVED_SEATS}""
fi

aleph-e2e-client "${ARGS}"

echo "Done!"
