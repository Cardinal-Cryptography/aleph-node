#!/bin/bash

# source assert.sh

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
# cd $CONTRACTS_PATH
# rustup show

cd $CONTRACTS_PATH/button-token
cargo contract build --release

cd $CONTRACTS_PATH/yellow-button
cargo contract build --release

## --- DEPLOY TOKEN CONTRACT
cd $CONTRACTS_PATH/button-token

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED)
BUTTON_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "button token contract address: " $BUTTON_TOKEN

## --- DEPLOY GAME CONTRACT
cd $CONTRACTS_PATH/yellow-button

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $ALICE_SEED)
YELLOW_BUTTON=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "game contract address: " $YELLOW_BUTTON

## --- TRANSFER BALANCE TO THE GAME CONTRACT

cd $CONTRACTS_PATH/button-token
cargo contract call --url $NODE --contract $BUTTON_TOKEN --message transfer --args $YELLOW_BUTTON $GAME_BALANCE --suri $ALICE_SEED

## -- MAKE READ ONLY CALLS
# cd $CONTRACTS_PATH/yellow-button

# cargo contract call --url $NODE --contract $YELLOW_BUTTON --message get_button_token --suri $ALICE_SEED
# cargo contract call --url $NODE --contract $YELLOW_BUTTON --message get_balance --suri $ALICE_SEED

## --- WHITELIST ACCOUNTS
cd $CONTRACTS_PATH/yellow-button

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message bulk_allow --args "[$ALICE,$NODE0]" --suri $ALICE_SEED

## --- PLAY
cd $CONTRACTS_PATH/yellow-button

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED

sleep 1

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $NODE0_SEED

## --- TRIGGER DEATH AND REWARDS DISTRIBUTION
cd $CONTRACTS_PATH/yellow-button

sleep 5

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED

## --- assert rewards distribution
cd $CONTRACTS_PATH/yellow-button

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message last_presser --suri $ALICE_SEED

# TODO

echo "Done"
exit $?
