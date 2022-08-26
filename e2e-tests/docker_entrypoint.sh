#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${RESERVED_SEATS}" || -z "${NON_RESERVED_SEATS}" ]]; then
  aleph-e2e-client --node "${NODE_URL}" --test-cases "${TEST_CASES}"
else
  aleph-e2e-client --node "${NODE_URL}" --test-cases "${TEST_CASES}" --reserved-seats "${RESERVED_SEATS}" \
    --non-reserved-seats "${NON_RESERVED_SEATS}"
fi

echo "Done!"
