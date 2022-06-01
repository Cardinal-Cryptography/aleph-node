#!/bin/bash

NODE=ws://127.0.0.1:9943
SEED=//Alice
CONTRACTS_PATH=/home/filip/CloudStation/aleph/aleph-node/contracts
CLI_PATH=/home/filip/CloudStation/aleph/aleph-node/bin/cliain


## --- DEPLOY GAME TOKEN

TOTAL_BALANCE=1000

cd $CONTRACTS_PATH/button-token
# cargo +nightly contract build --release

# CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $SEED)
# CONTRACT=$(cat out)
# BUTTON_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
BUTTON_TOKEN=5GmasQa5QBEPDYNpQye7Ht1pYmeBUceJQvwXZtCJUa3PS4mW
# CODE_HASH=$(echo "$CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-)

# echo "code hash: " $CODE_HASH
echo "contract address: " $BUTTON_TOKEN

## --- DEPLOY GAME CONTRACT

LIFETIME=900

cd $CONTRACTS_PATH/yellow-button
# cargo +nightly contract build --release

# CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BUTTON_TOKEN $LIFETIME --suri $SEED)
# YELLOW_BUTTON=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
YELLOW_BUTTON=5H9BynYoymMsCqqsX4f6iDi9MKfacL2mmEhVv6ByjnKfH1HM

echo "contract address: " $YELLOW_BUTTON

## --- TRANSFER ALL BALANCE TO THE GAME CONTRACT

cd $CONTRACTS_PATH/button-token

# cargo contract call --url $NODE --contract $BUTTON_TOKEN --message transfer --args $YELLOW_BUTTON $TOTAL_BALANCE --suri $SEED

## --- WHITELIST ACCOUNTS

cd $CONTRACTS_PATH/yellow-button

ALICE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
FILIP=5D2ZNVZ5xnMrs9SRJvVSe9ACnsbhTwmdC2PmeC1MXJVt8Drf

cargo contract call --url $NODE --contract $YELLOW_BUTTON --message bulk_allow --args "[$ALICE,$FILIP]" --suri $SEED

exit $?
