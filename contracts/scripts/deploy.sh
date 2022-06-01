#!/bin/bash

NODE=ws://127.0.0.1:9943

ALICE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
ALICE_SEED=//Alice

# NOTE: needs some balance
NODE1=5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o
NODE1_SEED=//1

LIFETIME=900
TOTAL_BALANCE=1000

CONTRACTS_PATH=/home/filip/CloudStation/aleph/aleph-node/contracts
CLI_PATH=/home/filip/CloudStation/aleph/aleph-node/bin/cliain

## --- DEPLOY GAME TOKEN

cd $CONTRACTS_PATH/button-token
cargo +nightly contract build --release

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED)
BUTTON_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
# BUTTON_TOKEN=5GmasQa5QBEPDYNpQye7Ht1pYmeBUceJQvwXZtCJUa3PS4mW
CODE_HASH=$(echo "$CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-)

echo "code hash: " $CODE_HASH
echo "contract address: " $BUTTON_TOKEN

## --- DEPLOY GAME CONTRACT

cd $CONTRACTS_PATH/yellow-button
cargo +nightly contract build --release

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $ALICE_SEED)
YELLOW_BUTTON=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
# YELLOW_BUTTON=5H9BynYoymMsCqqsX4f6iDi9MKfacL2mmEhVv6ByjnKfH1HM
CODE_HASH=$(echo "$CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-)

echo "code hash: " $CODE_HASH
echo "contract address: " $YELLOW_BUTTON

## --- TRANSFER ALL BALANCE TO THE GAME CONTRACT

cd $CONTRACTS_PATH/button-token

cargo contract call --url $NODE --contract $BUTTON_TOKEN --message transfer --args $YELLOW_BUTTON $TOTAL_BALANCE --suri $ALICE_SEED

## --- WHITELIST ACCOUNTS

cd $CONTRACTS_PATH/yellow-button

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message bulk_allow --args "[$ALICE,$NODE1]" --suri $ALICE_SEED

## --- PLAY

cd $CONTRACTS_PATH/yellow-button

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $ALICE_SEED

sleep 7

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message press --suri $NODE1_SEED

echo "Done"
exit $?
