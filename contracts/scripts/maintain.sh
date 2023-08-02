#!/bin/bash

## script for TheButton maintainance
## contains some functions e.g. to reset the games or top up the DEX with liquidity
## as well as some some useful queries
##
## should be used after sourcing env vars e.g.
## source ./contracts/env/dev && ./contracts/scripts/seed.sh

source $(dirname "$0")/common.sh

set -e # exit immediately if any command has a non-zero exit status
# set -x # print all executed commands to the terminal
set -o pipefail #  prevents errors in a pipeline from being masked

# --- GLOBAL CONSTANTS

INK_DEV_IMAGE=public.ecr.aws/p6e8q1z1/ink-dev:1.0.0
ADDRESSES_FILE=$(pwd)/contracts/addresses.mainnet.json
CONTRACTS_PATH=$(pwd)/contracts
#1e12
TOKEN_DECIMALS=1000000000000

# --- FUNCTIONS

function is_dead() {
  local game=$(get_address $1)
  cd "$CONTRACTS_PATH"/button
  cargo_contract call --url "$NODE" --contract "$game" --message is_dead --suri "$AUTHORITY_SEED" --dry-run --output-json | jq  -r '.data.Tuple.values' | jq '.[].Bool'
}

# this tx will reward ThePressiah and start a new round
function reset_game() {
  local game=$(get_address $1)
  cd "$CONTRACTS_PATH"/button
  cargo_contract call --url "$NODE" --contract "$game" --message reset --suri "$AUTHORITY_SEED" --skip-confirm
}

# this tx will only ThePressiah without starting the new round
function reward_pressiah() {
  local game=$(get_address $1)
  cd "$CONTRACTS_PATH"/button
  cargo_contract call --url "$NODE" --contract "$game" --message reward_pressiah --suri "$AUTHORITY_SEED" --skip-confirm
}

# returns wA0 balance of an address (denominated as 1e12)
function wazero_balance_of() {
  local account=$1

  cd "$CONTRACTS_PATH"/wrapped_azero
  local wrapped_azero=$(get_address wrapped_azero)
  local balance=$(cargo_contract call --url "$NODE" --contract "$wrapped_azero" --message PSP22::balance_of --args $account --suri "$AUTHORITY_SEED" --dry-run --output-json | jq  -r '.data.Tuple.values' | jq '.[].UInt')
  echo $(bc -l <<< $balance/$TOKEN_DECIMALS)
}

function round_number() {
  local game=$(get_address $1)
  cd "$CONTRACTS_PATH"/button
  cargo_contract call --url "$NODE" --contract "$game" --message round --suri "$AUTHORITY_SEED" --dry-run --output-json | jq  -r '.data.Tuple.values' | jq '.[].UInt'
}

# --- RUN

if [ -z "$AUTHORITY_SEED" ]; then
  echo "\$AUTHORITY_SEED is empty"
  exit -1
fi

run_ink_dev

# --- MAINTAN

if [ $(is_dead early_bird_special) == "true" ]; then
  echo "resetting EarlyBirdSpecial ..."
  # reset_game early_bird_special
fi

if [ $(is_dead back_to_the_future) == "true" ]; then
  echo "resetting BackToTheFuture ..."
  # reset_game back_to_the_future
fi


if [ $(is_dead the_pressiah_cometh) == "true" ]; then
  echo "resetting ThePressiahCometh ..."
  # reset_game the_pressiah_cometh
fi

current_balance=$(wazero_balance_of $(get_address simple_dex))
if (( $(echo "$current_balance < 1000" | bc -l) )); then
  echo "Adding liquidity to DEX ..."
  # add_liquidity
fi

# past some round, or when we distribute all A0 as rewards we likely want to reward the last pressiah and not reset the games anymore

# round_number early_bird_special
# round_number back_to_the_future
# round_number the_pressiah_cometh

# reward_pressiah early_bird_special
# reward_pressiah back_to_the_future
# reward_pressiah the_pressiah_comethx
