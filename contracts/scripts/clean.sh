#!/bin/bash

set -euo pipefail

# remove games (all 3)
# + remove code 
# remove ticket tokens (all 3)
# + remove code 
# remove reward tokens (all 3)
# + remove code 
# remove marketplaces (all 3)
# + remove code 

# remove access control
# + remove code

  # local contract_name=$1
  # local contract_address=$(cat "$CONTRACTS_PATH"/addresses.json | jq --raw-output ".$contract_name")
  # local ticket_address=$(cat "$CONTRACTS_PATH"/addresses.json | jq --raw-output ".${contract_name}_ticket")

# --- FUNCTIONS

function terminate_contract {
  local contract_address=$1 
  cargo contract call --url "$NODE" --contract $contract_address --message terminate --suri "$AUTHORITY_SEED"  
}

# remove code hash

# docker build --tag aleph-node:spec_version_6 -f ./docker/Dockerfile .
# public.ecr.aws/p6e8q1z1/cliain

docker run -e RUST_LOG=info "${CLIAIN_IMAGE}"

# --- GLOBAL CONSTANTS

CONTRACTS_PATH=$(pwd)/contracts
CLIAIN_IMAGE=public.ecr.aws/p6e8q1z1/cliain:latest

exit $?
