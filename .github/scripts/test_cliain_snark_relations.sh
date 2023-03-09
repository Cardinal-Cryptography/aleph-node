#!/usr/bin/env bash

set -euo pipefail

CLIAIN=./bin/cliain/target/release/cliain

# Test xor relation
${CLIAIN} snark-relation generate-keys xor
${CLIAIN} snark-relation generate-proof -p xor.groth16.pk.bytes xor -a 10 -b 11 -c 1
${CLIAIN} snark-relation verify \
  --verifying-key-file xor.groth16.vk.bytes \
  --proof-file xor.groth16.proof.bytes \
  --public-input-file xor.groth16.public_input.bytes

# Test linear equation relation
${CLIAIN} snark-relation generate-keys linear-equation
${CLIAIN} snark-relation generate-proof -p linear_equation.groth16.pk.bytes linear-equation
${CLIAIN} snark-relation verify \
  --verifying-key-file linear_equation.groth16.vk.bytes \
  --proof-file linear_equation.groth16.proof.bytes \
  --public-input-file linear_equation.groth16.public_input.bytes

# Test deposit relation
${CLIAIN} snark-relation generate-keys deposit
${CLIAIN} snark-relation generate-proof -p deposit.groth16.pk.bytes deposit \
  --note "2257517311045912551,9329547706917600007,17678219388335595033,2758194574870438734" \
  --token-id 1 \
  --token-amount 10 \
  --trapdoor 17 \
  --nullifier 19
${CLIAIN} snark-relation verify \
  --verifying-key-file deposit.groth16.vk.bytes \
  --proof-file deposit.groth16.proof.bytes \
  --public-input-file deposit.groth16.public_input.bytes
