#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function play {

  local contract_name=$1
  local contract_address=$2

  cd $CONTRACTS_PATH/$contract_name

  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::press --suri $PLAYER1_SEED

  sleep 1

  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::press --suri $PLAYER2_SEED

  # --- TRIGGER DEATH AND REWARDS DISTRIBUTION

  cd $CONTRACTS_PATH/$contract_name

  sleep $(($LIFETIME + 1))

  cargo contract call --url $NODE --contract $contract_address --message IButtonGame::death --suri $AUTHORITY_SEED

}

# --- ARGUMENTS

CONTRACTS_PATH=$(pwd)/contracts

# 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
PLAYER1_SEED=//0
# 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
PLAYER2_SEED=//Alice

EARLY_BIRD_SPECIAL=$(cat $CONTRACTS_PATH/addresses.json | jq --raw-output '.early_bird_special')
BACK_TO_THE_FUTURE=$(cat $CONTRACTS_PATH/addresses.json | jq --raw-output '.back_to_the_future')

# --- PLAY EARLY_BIRD_SPECIAL

play early_bird_special $EARLY_BIRD_SPECIAL

# --- PLAY BACK_TO_THE_FUTURE

play back_to_the_future $BACK_TO_THE_FUTURE

exit $?
