#!/bin/bash

## script to airdrop ticket tokens to the players
## should be used after sourcing env vars e.g.
## source ./contracts/env/dev && ./contracts/scripts/seed.sh

source $(dirname "$0")/common.sh

set -e # exit immediately if any command has a non-zero exit status
# set -x # print all executed commands to the terminal
set -o pipefail #  prevents errors in a pipeline from being masked

# --- GLOBAL CONSTANTS

# TODO : addresses.mainnet.json
ADDRESSES_FILE=$(pwd)/contracts/addresses.json
CONTRACTS_PATH=$(pwd)/contracts
PLAYERS_FILE=$(pwd)/contracts/scripts/stakers_mainnet.list
GAMES=(early_bird_special back_to_the_future the_pressiah_cometh)

# --- FUNCTIONS

# --- RUN

if [ -z "$AUTHORITY_SEED" ]; then
  echo "\$AUTHORITY_SEED is empty"
  exit -1
fi

run_ink_dev

# --- PERFORM AN AIRDROP

readarray -t PLAYERS < "$PLAYERS_FILE"

start=$(date +%s.%N)

for P in "${PLAYERS[@]}"; do

  echo "Sending tickets to ${P}"

  # --- send player ticket tokens for each game
  value=3
  cd "$CONTRACTS_PATH"/ticket_token
  for T in "${GAMES[@]}"; do

    echo -e "\tSending tickets foo ${T} ..."

    ticket_token_address=$(get_address ${T}_ticket)
    cargo_contract call --url "$NODE" --contract "$ticket_token_address" --message PSP22::transfer --args $P $value "[0]" --suri "$AUTHORITY_SEED" --skip-confirm
  done

done

end=`date +%s.%N`
echo "Airdrop done"
echo "Time elapsed: $( echo "$end - $start" | bc -l )"
