#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

source $(pwd)/.github/scripts/assert.sh

function link_bytecode() {
  local CONTRACT=$1
  local PLACEHOLDER=$2
  local REPLACEMENT=$3

  sed -i 's/'$PLACEHOLDER'/'$REPLACEMENT'/' target/ink/$CONTRACT.contract
}

# --- GLOBAL CONSTANTS

NODE_IMAGE=public.ecr.aws/p6e8q1z1/aleph-node:latest

NODE=ws://127.0.0.1:9943

ALICE=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
ALICE_SEED=//Alice

NODE0=5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
NODE0_SEED=//0

LIFETIME=5
TOTAL_BALANCE=1000
GAME_BALANCE=900

CONTRACTS_PATH=$(pwd)/contracts

# --- COMPILE CONTRACTS

cd $CONTRACTS_PATH/access_control
cargo contract build --release

cd $CONTRACTS_PATH/button_token
cargo contract build --release

cd $CONTRACTS_PATH/early_bird_special
cargo contract build --release

cd $CONTRACTS_PATH/back_to_the_future
cargo contract build --release

# --- DEPLOY ACCESS CONTROL CONTRACT

cd $CONTRACTS_PATH/access_control

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --suri $ALICE_SEED)
ACCESS_CONTROL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
ACCESS_CONTROL_PUBKEY=$(docker run --rm --entrypoint "/bin/sh" "${NODE_IMAGE}" -c "aleph-node key inspect $ACCESS_CONTROL" | grep hex | cut -c 23- | cut -c 3-)

echo "access control contract address: " $ACCESS_CONTROL
echo "access control contract public key (hex): " $ACCESS_CONTROL_PUBKEY

# --- UPLOAD TOKEN CONTRACT CODE

cd $CONTRACTS_PATH/button_token
# replace address placeholder with the on-chain address of the AccessControl contract
link_bytecode button_token 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
# remove just in case
rm target/ink/button_token.wasm
# NOTE : here we go from hex to binary using a nodejs cli tool
# availiable from https://github.com/fbielejec/polkadot-cljs
node ../scripts/hex-to-wasm.js target/ink/button_token.contract target/ink/button_token.wasm

CODE_HASH=$(cargo contract upload --url $NODE --suri $ALICE_SEED)
BUTTON_TOKEN_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "button token code hash" $BUTTON_TOKEN_CODE_HASH

# --- GRANT INIT PRIVILEDGES ON THE TOKEN CONTRACT CODE

cd $CONTRACTS_PATH/access_control

# alice is the initializer of the button-token contract
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Initializer('$BUTTON_TOKEN_CODE_HASH')' --suri $ALICE_SEED

#
# --- EARLY_BIRD_SPECIAL GAME
#

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE EARLY_BIRD_SPECIAL GAME

cd $CONTRACTS_PATH/button_token

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED --salt 0x4561726C79426972645370656369616C)
EARLY_BIRD_SPECIAL_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "EarlyBirdSpecial token contract instance address" $EARLY_BIRD_SPECIAL_TOKEN

## --- GRANT PRIVILEDGES ON THE EARLY_BIRD_SPECIAL TOKEN CONTRACT

cd $CONTRACTS_PATH/access_control

# alice is the admin and the owner of the contract instance
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Admin('$EARLY_BIRD_SPECIAL_TOKEN')' --suri $ALICE_SEED
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Owner('$EARLY_BIRD_SPECIAL_TOKEN')' --suri $ALICE_SEED

# --- UPLOAD EARLY_BIRD_SPECIAL GAME CONTRACT CODE

cd $CONTRACTS_PATH/early_bird_special
link_bytecode early_bird_special 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
rm target/ink/early_bird_special.wasm
node ../scripts/hex-to-wasm.js target/ink/early_bird_special.contract target/ink/early_bird_special.wasm

CODE_HASH=$(cargo contract upload --url $NODE --suri $ALICE_SEED)
EARLY_BIRD_SPECIAL_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "EarlyBirdSpecial contract code hash" $EARLY_BIRD_SPECIAL_CODE_HASH

# --- GRANT INIT PRIVILEDGES ON THE EARLY_BIRD_SPECIAL CONTRACT CODE

cd $CONTRACTS_PATH/access_control

cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Initializer('$EARLY_BIRD_SPECIAL_CODE_HASH')' --suri $ALICE_SEED

# --- CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME CONTRACT

cd $CONTRACTS_PATH/early_bird_special

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $EARLY_BIRD_SPECIAL_TOKEN $LIFETIME --suri $ALICE_SEED)
EARLY_BIRD_SPECIAL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "EarlyBirdSpecial contract instance address" $EARLY_BIRD_SPECIAL

# --- GRANT PRIVILEDGES ON THE EARLY_BIRD_SPECIAL GAME CONTRACT

cd $CONTRACTS_PATH/access_control

cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Owner('$EARLY_BIRD_SPECIAL')' --suri $ALICE_SEED
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Admin('$EARLY_BIRD_SPECIAL')' --suri $ALICE_SEED

## --- TRANSFER BALANCE TO THE EARLY_BIRD_SPECIAL GAME CONTRACT

cd $CONTRACTS_PATH/button_token

cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL_TOKEN --message transfer --args $EARLY_BIRD_SPECIAL $GAME_BALANCE --suri $ALICE_SEED

## --- WHITELIST ACCOUNTS

cd $CONTRACTS_PATH/early_bird_special

cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::bulk_allow --args "[$ALICE,$NODE0]" --suri $ALICE_SEED

## --- PLAY EARLY_BIRD_SPECIAL

cd $CONTRACTS_PATH/early_bird_special

cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::press --suri $ALICE_SEED

sleep 1

cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::press --suri $NODE0_SEED

## --- TRIGGER DEATH AND REWARDS DISTRIBUTION

cd $CONTRACTS_PATH/early_bird_special

sleep $(($LIFETIME + 1))

cargo contract call --url $NODE --contract $EARLY_BIRD_SPECIAL --message IButtonGame::death --suri $ALICE_SEED

#
# --- BACK_TO_THE_FUTURE GAME
#

# --- INITIALIZE TOKEN CONTRACT FOR THE BACK_TO_THE_FUTURE GAME

cd $CONTRACTS_PATH/button_token

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri $ALICE_SEED --salt 0x4261636B546F546865467574757265)
BACK_TO_THE_FUTURE_TOKEN=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "BackToTheFuture token contract instance address" $BACK_TO_THE_FUTURE_TOKEN

# --- GRANT PRIVILEDGES ON THE BACK_TO_THE_FUTURE TOKEN CONTRACT

cd $CONTRACTS_PATH/access_control

# alice is the admin and the owner of the contract instance
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Admin('$BACK_TO_THE_FUTURE_TOKEN')' --suri $ALICE_SEED
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Owner('$BACK_TO_THE_FUTURE_TOKEN')' --suri $ALICE_SEED

# --- UPLOAD BACK_TO_THE_FUTURE GAME CONTRACT

cd $CONTRACTS_PATH/back_to_the_future
link_bytecode back_to_the_future 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
rm target/ink/back_to_the_future.wasm
node ../scripts/hex-to-wasm.js target/ink/back_to_the_future.contract target/ink/back_to_the_future.wasm

CODE_HASH=$(cargo contract upload --url $NODE --suri $ALICE_SEED)
BACK_TO_THE_FUTURE_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "BackToTheFuture code hash" $BACK_TO_THE_FUTURE_CODE_HASH

# --- GRANT INIT PRIVILEDGES ON THE BACK_TO_THE_FUTURE GAME CONTRACT

# TODO : reverts

cd $CONTRACTS_PATH/access_control

cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Initializer('$BACK_TO_THE_FUTURE_CODE_HASH')' --suri $ALICE_SEED

# --- INITIALIZE BACK_TO_THE_FUTURE GAME CONTRACT

cd $CONTRACTS_PATH/back_to_the_future

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args $BACK_TO_THE_FUTURE_TOKEN $LIFETIME --suri $ALICE_SEED)
BACK_TO_THE_FUTURE=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)

echo "BackToTheFuture contract instance address" $BACK_TO_THE_FUTURE

# --- GRANT PRIVILEDGES ON THE BACK_TO_THE_FUTURE GAME CONTRACT

cd $CONTRACTS_PATH/access_control

cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Owner('$BACK_TO_THE_FUTURE')' --suri $ALICE_SEED
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $ALICE 'Admin('$BACK_TO_THE_FUTURE')' --suri $ALICE_SEED

# --- TRANSFER TOKENS TO THE BACK_TO_THE_FUTURE GAME CONTRACT

cd $CONTRACTS_PATH/button_token

cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE_TOKEN --message transfer --args $BACK_TO_THE_FUTURE $GAME_BALANCE --suri $ALICE_SEED

# --- WHITELIST ACCOUNTS FOR PLAYING

cd $CONTRACTS_PATH/back_to_the_future

cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::bulk_allow --args "[$ALICE,$NODE0]" --suri $ALICE_SEED

# --- PLAY BACK_TO_THE_FUTURE

cd $CONTRACTS_PATH/back_to_the_future

cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::press --suri $ALICE_SEED

sleep 1

cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::press --suri $NODE0_SEED

# --- TRIGGER DEATH AND REWARDS DISTRIBUTION

cd $CONTRACTS_PATH/back_to_the_future

sleep $(($LIFETIME + 1))

cargo contract call --url $NODE --contract $BACK_TO_THE_FUTURE --message IButtonGame::death --suri $ALICE_SEED

exit $?
