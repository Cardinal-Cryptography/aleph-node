#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function play {

  local contract_name=$1
  local contract_address=$(cat "$CONTRACTS_PATH"/addresses.json | jq --raw-output ".$contract_name")
  local ticket_address=$(cat "$CONTRACTS_PATH"/addresses.json | jq --raw-output ".${contract_name}_ticket")

  # airdrop initial tickets

  cd "$CONTRACTS_PATH"/ticket_token

  echo "sending ticket token" ${contract_name}_ticket "["$ticket_address"]" "to " $PLAYER1

  cargo contract call --url $NODE --contract $ticket_address --message PSP22::transfer --args $PLAYER1 1 "[0]" --suri $AUTHORITY_SEED --skip-confirm

  echo "sending ticket token" ${contract_name}_ticket "["$ticket_address"]" "to " $PLAYER2

  cargo contract call --url $NODE --contract $ticket_address --message PSP22::transfer --args $PLAYER2 1 "[0]" --suri $AUTHORITY_SEED --skip-confirm

  # give allowance for spending tickets to the game contract

  echo "allowing" $contract_name "["$contract_address"]" "to spend up to" $TICKET_BALANCE "of" ${contract_name}_ticket "["$ticket_address"]" "on behalf of" $PLAYER1

  cargo contract call --url $NODE --contract $ticket_address --message PSP22::approve --args $contract_address $TICKET_BALANCE --suri $PLAYER1_SEED --skip-confirm

  echo "allowing" $contract_name "["$contract_address"]" "to spend up to" $TICKET_BALANCE "of" ${contract_name}_ticket "["$ticket_address"]" "on behalf of" $PLAYER2

  cargo contract call --url $NODE --contract $ticket_address --message PSP22::approve --args $contract_address $TICKET_BALANCE --suri $PLAYER2_SEED --skip-confirm

# TODO: uncomment when cargo contract doesn't break on parsing "foreign" events
#
#  # play the game
#
#  cd "$CONTRACTS_PATH"/button
#
#  echo "calling press for" $contract_name "["$contract_address"]" "by" $PLAYER1_SEED
#
#  cargo contract call --url $NODE --contract $contract_address --message press --suri $PLAYER1_SEED --skip-confirm
#
#  sleep 1
#
#  echo "calling press for" $contract_name "["$contract_address "]" "by" $PLAYER2_SEED
#
#  cargo contract call --url $NODE --contract $contract_address --message press --suri $PLAYER2_SEED --skip-confirm
#
#  # ---  WAIT FOR THE BUTTON DEATH
#
#  sleep $(($LIFETIME + 1))
#
#  # --- TRIGGER GAME RESET
#
#  cargo contract call --url $NODE --contract $contract_address --message reset --suri $AUTHORITY_SEED --skip-confirm
#
#  echo "Done playing" $contract_name
}

# --- ARGUMENTS

CONTRACTS_PATH=$(pwd)/contracts

PLAYER1=5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
PLAYER1_SEED=//0
PLAYER2=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
PLAYER2_SEED=//Alice

GAMES=(early_bird_special back_to_the_future the_pressiah_cometh)
for GAME in "${GAMES[@]}"; do
  (
    play $GAME
  )&
done

exit $?
