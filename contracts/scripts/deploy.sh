#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function instrument_ticket_token {

  local  __resultvar=$1
  local contract_name=$2
  local salt=$3
  local token_name=$4
  local token_symbol=$5

  # --- CREATE AN INSTANCE OF THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/$contract_name

  local contract_address=$(cargo contract instantiate --url $NODE --constructor new --args \"$token_name\" \"$token_symbol\" $TOTAL_BALANCE --suri "$AUTHORITY_SEED" --salt $salt)

  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo $contract_name "ticket contract instance address: " $contract_address

  # --- GRANT PRIVILEDGES ON THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  # set the admin and the owner of the contract instance
  cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Owner('$contract_address')' --suri "$AUTHORITY_SEED"

  eval $__resultvar="'$contract_address'"
}

function instrument_game_token {

  local  __resultvar=$1
  local contract_name=$2
  local salt=$3

  # --- CREATE AN INSTANCE OF THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/$contract_name

  # TODO : remove balance when token is mintable
  local contract_address=$(cargo contract instantiate --url $NODE --constructor new --args $TOTAL_BALANCE --suri "$AUTHORITY_SEED" --salt $salt)
  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo $contract_name "token contract instance address: " $contract_address

  # --- GRANT PRIVILEDGES ON THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  # set the owner of the contract instance
  cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Owner('$contract_address')' --suri "$AUTHORITY_SEED"

  # TODO : MINTER / BURNER roles

  eval $__resultvar="'$contract_address'"
}

function deploy_and_instrument_game {

  local  __resultvar=$1
  local contract_name=$2
  local ticket_token=$3
  local game_token=$4

  # --- UPLOAD CONTRACT CODE

  cd "$CONTRACTS_PATH"/$contract_name
  link_bytecode $contract_name 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
  rm target/ink/$contract_name.wasm
  node ../scripts/hex-to-wasm.js target/ink/$contract_name.contract target/ink/$contract_name.wasm

  local code_hash=$(cargo contract upload --url $NODE --suri "$AUTHORITY_SEED")
  local code_hash=$(echo "$code_hash" | grep hash | tail -1 | cut -c 15-)

  # --- GRANT INIT PRIVILEDGES ON THE CONTRACT CODE

  cd "$CONTRACTS_PATH"/access_control

  cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Initializer('$code_hash')' --suri "$AUTHORITY_SEED"

  # --- CREATE AN INSTANCE OF THE CONTRACT

  cd "$CONTRACTS_PATH"/$contract_name

  local contract_address=$(cargo contract instantiate --url $NODE --constructor new --args $ticket_token $game_token $LIFETIME --suri "$AUTHORITY_SEED")
  local contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo $contract_name "contract instance address: " $contract_address

  # --- GRANT PRIVILEDGES ON THE CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Owner('$contract_address')' --suri "$AUTHORITY_SEED"

  eval $__resultvar="'$contract_address'"
}

function link_bytecode() {
  local contract=$1
  local placeholder=$2
  local replacement=$3

  sed -i 's/'$placeholder'/'$replacement'/' target/ink/$contract.contract
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

cd "$CONTRACTS_PATH"/early_bird_special
cargo contract build --release

cd "$CONTRACTS_PATH"/back_to_the_future
cargo contract build --release

cd "$CONTRACTS_PATH"/the_pressiah_cometh
cargo contract build --release

# --- DEPLOY ACCESS CONTROL CONTRACT

cd "$CONTRACTS_PATH"/access_control

CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --suri "$AUTHORITY_SEED")
ACCESS_CONTROL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
ACCESS_CONTROL_PUBKEY=$(docker run --rm --entrypoint "/bin/sh" "${NODE_IMAGE}" -c "aleph-node key inspect $ACCESS_CONTROL" | grep hex | cut -c 23- | cut -c 3-)

echo "access control contract address: " $ACCESS_CONTROL
echo "access control contract public key (hex): " $ACCESS_CONTROL_PUBKEY

# --- UPLOAD TICKET TOKEN CONTRACT CODE

cd "$CONTRACTS_PATH"/ticket_token
# replace address placeholder with the on-chain address of the AccessControl contract
link_bytecode ticket_token 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
# remove just in case
rm target/ink/ticket_token.wasm
# NOTE : here we go from hex to binary using a nodejs cli tool
# availiable from https://github.com/fbielejec/polkadot-cljs
node ../scripts/hex-to-wasm.js target/ink/ticket_token.contract target/ink/ticket_token.wasm

CODE_HASH=$(cargo contract upload --url $NODE --suri "$AUTHORITY_SEED")
TICKET_TOKEN_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "ticket token code hash" $TICKET_TOKEN_CODE_HASH

# --- UPLOAD REWARD TOKEN CONTRACT CODE

cd "$CONTRACTS_PATH"/game_token
# replace address placeholder with the on-chain address of the AccessControl contract
link_bytecode game_token 4465614444656144446561444465614444656144446561444465614444656144 $ACCESS_CONTROL_PUBKEY
# remove just in case
rm target/ink/game_token.wasm
# NOTE : here we go from hex to binary using a nodejs cli tool
# availiable from https://github.com/fbielejec/polkadot-cljs
node ../scripts/hex-to-wasm.js target/ink/game_token.contract target/ink/game_token.wasm

CODE_HASH=$(cargo contract upload --url $NODE --suri "$AUTHORITY_SEED")
GAME_TOKEN_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "button token code hash" $GAME_TOKEN_CODE_HASH

# --- GRANT INIT PRIVILEDGES ON THE TOKEN AND TICKET CONTRACT CODE

cd "$CONTRACTS_PATH"/access_control

# set the initializer of the token contract
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Initializer('$GAME_TOKEN_CODE_HASH')' --suri "$AUTHORITY_SEED"

# set the initializer of the ticket contract
cargo contract call --url $NODE --contract $ACCESS_CONTROL --message grant_role --args $AUTHORITY 'Initializer('$TICKET_TOKEN_CODE_HASH')' --suri "$AUTHORITY_SEED"

start=`date +%s.%N`

#
# --- EARLY_BIRD_SPECIAL GAME
#

# --- CREATE AN INSTANCE OF THE TICKET CONTRACT FOR THE EARLY_BIRD_SPECIAL GAME

instrument_ticket_token EARLY_BIRD_SPECIAL_TICKET ticket_token 0x4561726C79426972645370656369616C early_bird_special EBS

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE EARLY_BIRD_SPECIAL GAME

instrument_game_token EARLY_BIRD_SPECIAL_TOKEN game_token 0x4561726C79426972645370656369616C

# --- UPLOAD CODE AND CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME

deploy_and_instrument_game EARLY_BIRD_SPECIAL early_bird_special $EARLY_BIRD_SPECIAL_TICKET $EARLY_BIRD_SPECIAL_TOKEN

#
# --- BACK_TO_THE_FUTURE GAME
#

# --- CREATE AN INSTANCE OF THE TICKET CONTRACT FOR THE BACK_TO_THE_FUTURE GAME

instrument_ticket_token BACK_TO_THE_FUTURE_TICKET ticket_token 0x4261636B546F546865467574757265 back_to_the_future BTF

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE BACK_TO_THE_FUTURE GAME

instrument_game_token BACK_TO_THE_FUTURE_TOKEN game_token 0x4261636B546F546865467574757265

# --- UPLOAD CODE AND CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME

deploy_and_instrument_game BACK_TO_THE_FUTURE back_to_the_future $BACK_TO_THE_FUTURE_TICKET $BACK_TO_THE_FUTURE_TOKEN

#
# --- THE_PRESSIAH_COMETH GAME
#

# --- CREATE AN INSTANCE OF THE TICKET CONTRACT FOR THE THE_PRESSIAH_COMETH GAME

instrument_ticket_token THE_PRESSIAH_COMETH_TICKET ticket_token 0x4261636B546F546865467574752137 back_to_the_future BTF

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE THE_PRESSIAH_COMETH GAME

instrument_game_token THE_PRESSIAH_COMETH_TOKEN game_token 0x4261636B546F546865467574752137

# --- UPLOAD CODE AND CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME

deploy_and_instrument_game THE_PRESSIAH_COMETH the_pressiah_cometh $THE_PRESSIAH_COMETH_TICKET $THE_PRESSIAH_COMETH_TOKEN

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
echo "Time elapsed:" $( echo "$end - $start" | bc -l )

exit $?
