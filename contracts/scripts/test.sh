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

# 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
PLAYER1_SEED=//0
# 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
PLAYER2_SEED=//Alice

# --- PLAY EARLY_BIRD_SPECIAL

# TODO get address from deploy step output filed

play early_bird_special $EARLY_BIRD_SPECIAL

# cd $CONTRACTS_PATH/early_bird_special

# cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::press --suri $AUTHORITY_SEED

# sleep 1

# cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::press --suri $NODE0_SEED

# # --- TRIGGER DEATH AND REWARDS DISTRIBUTION

# cd $CONTRACTS_PATH/early_bird_special

# sleep $(($LIFETIME + 1))

# cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::death --suri $AUTHORITY_SEED

# --- PLAY BACK_TO_THE_FUTURE

# TODO get address from deploy step output filed
play back_to_the_future $BACK_TO_THE_FUTURE

# cd $CONTRACTS_PATH/back_to_the_future

# cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::press --suri $AUTHORITY_SEED

# sleep 1

# cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::press --suri $NODE0_SEED

# # --- TRIGGER DEATH AND REWARDS DISTRIBUTION

# cd $CONTRACTS_PATH/back_to_the_future

# sleep $(($LIFETIME + 1))

# cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::death --suri $AUTHORITY_SEED

exit $?
