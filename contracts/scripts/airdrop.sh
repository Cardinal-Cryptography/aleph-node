#!/bin/bash

## script to airdrop ticket tokens to the players
## should be used after sourcing env vars e.g.
## source ./contracts/env/dev && ./contracts/scripts/seed.sh

source $(dirname "$0")/common.sh

set -e # exit immediately if any command has a non-zero exit status
# set -x # print all executed commands to the terminal
set -o pipefail #  prevents errors in a pipeline from being masked

# --- GLOBAL CONSTANTS

ADDRESSES_FILE=$(pwd)/contracts/addresses.mainnet.json
CONTRACTS_PATH=$(pwd)/contracts
PLAYERS_FILE=$(pwd)/contracts/scripts/stakers_mainnet.list
GAMES=(early_bird_special back_to_the_future the_pressiah_cometh)

# --- FUNCTIONS

# --- RUN

if [ -z "$AUTHORITY_SEED" ]; then
  echo "\$AUTHORITY_SEED is empty"
  exit -1
fi

# --- PERFORM THE AIRDROP

start=$(date +%s.%N)

for T in "${GAMES[@]}"; do

  echo -e "\tSending tickets for ${T} ..."
  ticket_token_address=$(get_address ${T}_ticket)
  python3 $PWD/contracts/scripts/airdrop.py -p $PLAYERS_FILE -s "$AUTHORITY_SEED" --node $NODE -c $ticket_token_address -m $CONTRACTS_PATH/ticket_token/target/ink/ticket_token.json

done

end=`date +%s.%N`
echo "Airdrop done"
echo "Time elapsed: $( echo "$end - $start" | bc -l )"
