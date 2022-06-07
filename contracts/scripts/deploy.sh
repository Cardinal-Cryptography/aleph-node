#!/bin/bash

NODE=ws://127.0.0.1:9943

ALICE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
ALICE_SEED=//Alice

# NOTE: needs some balance
NODE0=5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
NODE0_SEED=//0

LIFETIME=5
TOTAL_BALANCE=1000
GAME_BALANCE=900

# CONTRACTS_PATH=/home/filip/CloudStation/aleph/aleph-node/contracts
CONTRACTS_PATH=$(pwd)/contracts

## --- DEPLOY TOKEN CONTRACT

cd $CONTRACTS_PATH/button-token
cargo +nightly contract build --release

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED)
BUTTON_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
# BUTTON_TOKEN=5C6tBqxFKoPVhGfAzgR8JGMrMZNJy7pDjB5gvrs1z5mPscKX
# CODE_HASH=$(echo "$CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-)

# echo "code hash: " $CODE_HASH
echo "contract address: " $BUTTON_TOKEN

## --- DEPLOY GAME CONTRACT

cd $CONTRACTS_PATH/yellow-button
cargo +nightly contract build --release

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $ALICE_SEED)
# CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $ALICE_SEED --code-hash )

YELLOW_BUTTON=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
# YELLOW_BUTTON=5H9BynYoymMsCqqsX4f6iDi9MKfacL2mmEhVv6ByjnKfH1HM
CODE_HASH=$(echo "$CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-)

echo "code hash: " $CODE_HASH
echo "contract address: " $YELLOW_BUTTON

## --- TRANSFER BALANCE TO THE GAME CONTRACT

cd $CONTRACTS_PATH/button-token

cargo contract call --url $NODE --contract $BUTTON_TOKEN --message transfer --args $YELLOW_BUTTON $GAME_BALANCE --suri $ALICE_SEED

## -- DEBUGGING CALLS

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

## --- TRIGGER DEATH AND REWARD DISTRIBUTION

sleep 5

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED

## --- TODO : assert rewards distribution

# 


echo "Done"
exit $?
