#!/usr/bin/env bash

set -euo pipefail

CLIAIN=./bin/cliain/target/release/cliain

# Test xor relation
CLIAIN snark-relation generate-keys xor
CLIAIN snark-relation generate-proof -p xor.groth16.pk.bytes xor -a 11 -b 11 -c 1
CLIAIN snark-relation verify \
  --verifying-key-file xor.groth16.vk.bytes \
  --proof-file xor.groth16.proof.bytes \
  --public-input-file xor.groth16.public_input.bytes
