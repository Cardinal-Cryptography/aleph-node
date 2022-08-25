#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${RESERVED_SEATS}" && -n "${NON_RESERVED_SEATS}" ]]; then
  aleph-e2e-client --node "${NODE_URL}" --test-cases "${TEST_CASES}" --reserved-seats "${RESERVED_SEATS}" \
    --non-reserved-seats "${NON_RESERVED_SEATS}"
else
  aleph-e2e-client --node "${NODE_URL}" --test-cases "${TEST_CASES}"
fi

echo "Done!"
