#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

source $(pwd)/.github/scripts/assert.sh

function link_bytecode() {
  CONTRACT=$1
  PLACEHOLDER=$2
  REPLACEMENT=$3

  sed -i 's/$PLACEHOLDER/$REPLACEMENT/' target/ink/$CONTRACT.contract  
}

# --- CONSTANTS

NODE=ws://127.0.0.1:9943

ALICE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
ALICE_SEED=//Alice

NODE0=5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
NODE0_SEED=//0

LIFETIME=5
TOTAL_BALANCE=1000
GAME_BALANCE=900

CONTRACTS_PATH=$(pwd)/contracts

## --- COMPILE CONTRACTS

cd $CONTRACTS_PATH/access-control
cargo contract build --release

cd $CONTRACTS_PATH/button-token
cargo contract build --release

cd $CONTRACTS_PATH/yellow-button
cargo contract build --release

## --- DEPLOY ACCESS CONTROL CONTRACT
cd $CONTRACTS_PATH/access-control

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --suri $ALICE_SEED)
ACCESS_CONTROL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

## --- DEPLOY TOKEN CONTRACT
cd $CONTRACTS_PATH/button-token
link_bytecode button_token 4465614444656144446561444465614444656144446561444465614444656144 c8289e01bb596595ebb6135366f9a0705b51c305108ccd466cad810a020fd5b8

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED)
BUTTON_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
BUTTON_TOKEN_CODE_HASH=$(echo "$CONTRACT" | grep hash | tail -1 | cut -c 15-)

echo "button token contract address: " $BUTTON_TOKEN
echo "button token code hash:        " $BUTTON_TOKEN_CODE_HASH

## --- GRANT PROVILEDGES



# ## --- DEPLOY GAME CONTRACT
# cd $CONTRACTS_PATH/yellow-button

# CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $ALICE_SEED)
# YELLOW_BUTTON=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

# echo "game contract address: " $YELLOW_BUTTON

# ## --- TRANSFER BALANCE TO THE GAME CONTRACT

# cd $CONTRACTS_PATH/button-token
# cargo contract call --url $NODE --contract $BUTTON_TOKEN --message transfer --args $YELLOW_BUTTON $GAME_BALANCE --suri $ALICE_SEED

# ## --- WHITELIST ACCOUNTS
# cd $CONTRACTS_PATH/yellow-button

# cargo contract call --url $NODE --contract $YELLOW_BUTTON --message bulk_allow --args "[$ALICE,$NODE0]" --suri $ALICE_SEED

# ## --- PLAY
# cd $CONTRACTS_PATH/yellow-button

# cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED

# sleep 1

# cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $NODE0_SEED

# ## --- TRIGGER DEATH AND REWARDS DISTRIBUTION
# cd $CONTRACTS_PATH/yellow-button

# sleep $(($LIFETIME + 1))

# EVENT=$(cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED | grep ButtonDeath)
# EVENT=$(echo "$EVENT" | sed 's/^ *//g' | tr " " "\n")

# PRESSIAH_REWARD=$(echo "$EVENT" | sed -n '7p' | tail -1)
# PRESSIAH_REWARD=${PRESSIAH_REWARD::-1}

# echo "The Pressiah receives: $PRESSIAH_REWARD"
# assert_eq "450" "$PRESSIAH_REWARD"

# exit $?
