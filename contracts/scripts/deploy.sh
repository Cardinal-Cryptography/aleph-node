#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function upload_contract {

  local  __resultvar=$1
  local contract_name=$2

  cd "$CONTRACTS_PATH"/$contract_name

  # --- REPLACE THE ADDRESS OF ACCESS CONTROL CONTRACT

  # replace address placeholder with the on-chain address of the AccessControl contract
  link_bytecode $contract_name 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
  # remove just in case
  rm target/ink/$contract_name.wasm
  # NOTE : here we go from hex to binary using a nodejs cli tool
  # availiable from https://github.com/fbielejec/polkadot-cljs
  node ../scripts/hex-to-wasm.js target/ink/$contract_name.contract target/ink/$contract_name.wasm

  # --- UPLOAD CONTRACT CODE

  code_hash=$(cargo contract upload --url "$NODE" --suri "$AUTHORITY_SEED")
  code_hash=$(echo "$code_hash" | grep hash | tail -1 | cut -c 15-)

  echo "$contract_name code hash: $code_hash"

  cd "$CONTRACTS_PATH"/access_control

  # Set the initializer of the contract
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Initializer('"$code_hash"')' --suri "$AUTHORITY_SEED"

  eval $__resultvar="'$code_hash'"
}

function deploy_ticket_token {

  local  __resultvar=$1
  local token_name=$2
  local token_symbol=$3
  local salt=$4

  # --- CREATE AN INSTANCE OF THE TICKET CONTRACT

  cd "$CONTRACTS_PATH"/ticket_token

  local contract_address=$(cargo contract instantiate --url "$NODE" --constructor new --args \"$token_name\" \"$token_symbol\" "$TOTAL_BALANCE" --suri "$AUTHORITY_SEED" --salt "$salt")
  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo "$token_symbol ticket contract instance address:  $contract_address"

  # --- GRANT PRIVILEGES ON THE TICKET CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  # set the admin and the owner of the contract instance
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Owner('"$contract_address"')' --suri "$AUTHORITY_SEED"

  eval $__resultvar="'$contract_address'"
}


function deploy_game_token {

  local  __resultvar=$1
  local token_name=$2
  local token_symbol=$3
  local salt=$4

  # --- CREATE AN INSTANCE OF THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/game_token

  # TODO : remove balance when token is mintable
  local contract_address=$(cargo contract instantiate --url "$NODE" --constructor new --args \"$token_name\" \"$token_symbol\" "$TOTAL_BALANCE" --suri "$AUTHORITY_SEED" --salt "$salt")
  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo "$token_symbol token contract instance address: $contract_address"

  # --- GRANT PRIVILEGES ON THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  # set the owner of the contract instance
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Owner('"$contract_address"')' --suri "$AUTHORITY_SEED"

  # TODO : MINTER / BURNER roles

  eval "$__resultvar='$contract_address'"
}


function deploy_button_game {

  local  __resultvar=$1
  local game_type=$2
  local ticket_token=$3
  local game_token=$4
  local salt=$5

  # --- CREATE AN INSTANCE OF THE CONTRACT

  cd "$CONTRACTS_PATH"/button

  local contract_address=$(cargo contract instantiate --url "$NODE" --constructor new --args "$ticket_token" "$game_token" "$LIFETIME" "$game_type" --suri "$AUTHORITY_SEED" --salt "$salt")
  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo "$game_type contract instance address: $contract_address"

  # --- GRANT PRIVILEGES ON THE CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Owner('"$contract_address"')' --suri "$AUTHORITY_SEED"

  eval "$__resultvar='$contract_address'"
}


function link_bytecode() {
  local contract=$1
  local placeholder=$2
  local replacement=$3

  sed -i 's/'"$placeholder"'/'"$replacement"'/' "target/ink/$contract.contract"
}


# --- GLOBAL CONSTANTS

NODE_IMAGE=public.ecr.aws/p6e8q1z1/aleph-node:latest

CONTRACTS_PATH=$(pwd)/contracts


# --- COMPILE CONTRACTS

cd "$CONTRACTS_PATH"/access_control
cargo contract build --release

cd "$CONTRACTS_PATH"/ticket_token
cargo contract build --release

cd "$CONTRACTS_PATH"/game_token
cargo contract build --release

cd "$CONTRACTS_PATH"/button
cargo contract build --release


# --- DEPLOY ACCESS CONTROL CONTRACT

cd "$CONTRACTS_PATH"/access_control

CONTRACT=$(cargo contract instantiate --url "$NODE" --constructor new --suri "$AUTHORITY_SEED")
ACCESS_CONTROL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
ACCESS_CONTROL_PUBKEY=$(docker run --rm --entrypoint "/bin/sh" "${NODE_IMAGE}" -c "aleph-node key inspect $ACCESS_CONTROL" | grep hex | cut -c 23- | cut -c 3-)

echo "access control contract address: $ACCESS_CONTROL"
echo "access control contract public key \(hex\): $ACCESS_CONTROL_PUBKEY"


# --- UPLOAD CONTRACTS CODES

upload_contract TICKET_TOKEN_CODE_HASH ticket_token
upload_contract GAME_TOKEN_CODE_HASH game_token
upload_contract BUTTON_CODE_HASH button


start=`date +%s.%N`

#
# --- EARLY_BIRD_SPECIAL GAME
#
echo "Early Bird Special"

salt="0x4561726C79426972645370656369616C"
deploy_ticket_token EARLY_BIRD_SPECIAL_TICKET early_bird_special_ticket EBST $salt
deploy_game_token EARLY_BIRD_SPECIAL_TOKEN early_bird_special EBS $salt
deploy_button_game EARLY_BIRD_SPECIAL EarlyBirdSpecial $EARLY_BIRD_SPECIAL_TICKET $EARLY_BIRD_SPECIAL_TOKEN $salt

#
# --- BACK_TO_THE_FUTURE GAME
#
echo "Back To The Future"

salt="0x4261636B546F546865467574757265"
deploy_ticket_token BACK_TO_THE_FUTURE_TICKET back_to_the_future_ticket BTFT $salt
deploy_game_token BACK_TO_THE_FUTURE_TOKEN back_to_the_future BTF $salt
deploy_button_game BACK_TO_THE_FUTURE BackToTheFuture $BACK_TO_THE_FUTURE_TICKET $BACK_TO_THE_FUTURE_TOKEN $salt

#
# --- THE_PRESSIAH_COMETH GAME
#
echo "The Pressiah Cometh"

salt="0x7468655F70726573736961685F636F6D657468"
deploy_ticket_token THE_PRESSIAH_COMETH_TICKET the_pressiah_cometh_ticket TPCT $salt
deploy_game_token THE_PRESSIAH_COMETH_TOKEN the_pressiah_cometh TPC $salt
deploy_button_game THE_PRESSIAH_COMETH ThePressiahCometh $THE_PRESSIAH_COMETH_TICKET $THE_PRESSIAH_COMETH_TOKEN $salt


# spit adresses to a JSON file
cd "$CONTRACTS_PATH"

jq -n --arg early_bird_special $EARLY_BIRD_SPECIAL \
   --arg early_bird_special_ticket $EARLY_BIRD_SPECIAL_TICKET \
   --arg early_bird_special_token $EARLY_BIRD_SPECIAL_TOKEN \
   --arg back_to_the_future $BACK_TO_THE_FUTURE \
   --arg back_to_the_future_ticket $BACK_TO_THE_FUTURE_TICKET \
   --arg back_to_the_future_token $BACK_TO_THE_FUTURE_TOKEN \
   --arg the_pressiah_cometh $THE_PRESSIAH_COMETH \
   --arg the_pressiah_cometh_ticket $THE_PRESSIAH_COMETH_TICKET \
   --arg the_pressiah_cometh_token $THE_PRESSIAH_COMETH_TOKEN \
   '{early_bird_special: $early_bird_special,
     early_bird_special_ticket: $early_bird_special_ticket,
     early_bird_special_token: $early_bird_special_token,
     back_to_the_future: $back_to_the_future,
     back_to_the_future_ticket: $back_to_the_future_ticket,
     back_to_the_future_token: $back_to_the_future_token,
     the_pressiah_cometh: $the_pressiah_cometh,
     the_pressiah_cometh_ticket: $the_pressiah_cometh_ticket,
     the_pressiah_cometh_token: $the_pressiah_cometh_token}' > addresses.json


end=`date +%s.%N`
echo "Time elapsed: $( echo "$end - $start" | bc -l )"

exit $?
