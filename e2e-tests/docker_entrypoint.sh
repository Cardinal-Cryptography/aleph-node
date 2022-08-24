#!/usr/bin/env bash
set -euo pipefail

aleph-e2e-client --node "${NODE_URL}" --test-cases "${TEST_CASES}" --min-validator-count "${MIN_VALIDATOR_COUNT}" \
  --reserved-seats "${RESERVED_SEATS}" --non-reserved-seats "${NON_RESERVED_SEATS}"

echo "Done!"
