#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function play {

  local contract_name=$1
  local contract_address=$(cat $CONTRACTS_PATH/addresses.json | jq --raw-output ".$contract_name")

  cd $CONTRACTS_PATH/$contract_name

  echo "@ calling press for" $contract_name "@" $contract_address "by" $PLAYER1_SEED

  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::press --suri $PLAYER1_SEED

  sleep 1

  echo "@ calling press for" $contract_name "@" $contract_address "by" $PLAYER2_SEED

  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::press --suri $PLAYER2_SEED

  # --- TRIGGER DEATH AND REWARDS DISTRIBUTION

  sleep $(($LIFETIME + 1))

  echo "@ calling death for" $contract_name
  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::death --suri $AUTHORITY_SEED
}

# --- ARGUMENTS

CONTRACTS_PATH=$(pwd)/contracts

# 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
PLAYER1_SEED=//0
# 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
PLAYER2_SEED=//Alice

GAMES=(early_bird_special back_to_the_future)
# GAMES=(back_to_the_future)
# GAMES=(early_bird_special)
for GAME in "${GAMES[@]}"; do
  (
    play $GAME
  )&
done

exit $?
