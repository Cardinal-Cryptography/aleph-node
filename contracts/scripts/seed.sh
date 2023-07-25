#!/bin/bash

## script to populate TheButton with some initial state
## should be used after sourcing env vars e.g.
## source ./contracts/env/dev && ./contracts/scripts/seed.sh

source $(dirname "$0")/common.sh


set -e # exit immediatley if any command has a non-zero exit status
set -x # print all executed commands to the terminal
set -o pipefail #  prevents errors in a pipeline from being masked

# TODO : addresses.mainnet.json
ADDRESSES_FILE=$(pwd)/contracts/addresses.json
CONTRACTS_PATH=$(pwd)/contracts

# --- FUNCTIONS

function get_address {
  local contract_name=$1
  cat $ADDRESSES_FILE | jq --raw-output ".$contract_name"
}

# --- RUN

if [ -z "$AUTHORITY_SEED" ]; then
      echo "\$AUTHORITY_SEED is empty"
      exit -1
fi

run_ink_dev

# --- SEED GAMES STATE

ACCESS_CONTROL=$(get_address access_control)
DEX=$(get_address simple_dex)
WRAPPED_AZERO=$(get_address wrapped_azero)
GAMES=(early_bird_special back_to_the_future the_pressiah_cometh)

for T in "${GAMES[@]}"; do
  game_token_address=$(get_address ${T}_token)

  # --- give the authority mint rights to the game token
  cd "$CONTRACTS_PATH"/access_control
  cargo_contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Custom('"$game_token_address"',[0x4D,0x49,0x4E,0x54])' --suri "$AUTHORITY_SEED" --skip-confirm

  # --- mint some tokens directly to the DEX
  cd "$CONTRACTS_PATH"/game_token

  amount=1000000000000000 # 1000 tokens
  if [ $T -eq 'the_pressiah_cometh']; then
    amount=10000000000000 # 10 tokens
  fi

  cargo_contract call --url "$NODE" --contract "$game_token_address" --message PSP22Mintable::mint --args $DEX $amount --suri "$AUTHORITY_SEED" --skip-confirm

  # --- send ticket tokens to the Marketplace

  ticket_token_address=$(get_address ${T}_ticket)
  marketplace_address=$(get_address ${T}_marketplace)
  # put this many tickets up for sale on the marketplace
  value=1000

  cd "$CONTRACTS_PATH"/ticket_token
  cargo_contract call --url "$NODE" --contract "$ticket_token_address" --message PSP22::transfer --args $marketplace_address $value "[0]" --suri "$AUTHORITY_SEED" --skip-confirm

done

# --- provide DEX with wA0 liquidity
value=1000000000000000 # 1k A0

cd "$CONTRACTS_PATH"/wrapped_azero
# --- wrap some AZERO
cargo_contract call --url "$NODE" --contract "$WRAPPED_AZERO" --message wrap --value $value --suri "$AUTHORITY_SEED" --skip-confirm
# --- send it to the DEX
cargo_contract call --url "$NODE" --contract "$WRAPPED_AZERO" --message PSP22::transfer --args $DEX $value "[0]" --suri "$AUTHORITY_SEED" --skip-confirm

echo "Games: Done"
