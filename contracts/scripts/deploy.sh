#!/bin/bash

set -euo pipefail

# --- FUNCTIONS

function instrument_game_token {

  local  __resultvar=$1
  local contract_name=$2
  local token_name=\"$3\"
  local token_symbol=\"$4\"
  local salt=$5

  # --- CREATE AN INSTANCE OF THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/"$contract_name"

  local contract_address
  contract_address=$(cargo contract instantiate --url "$NODE" --constructor new --args "$token_name" "$token_symbol" "$TOTAL_BALANCE" --suri "$AUTHORITY_SEED" --salt "$salt")
  contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo "$contract_name token contract instance address: $contract_address"

  # --- GRANT PRIVILEDGES ON THE TOKEN CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  # set the admin and the owner of the contract instance
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Admin('"$contract_address"')' --suri "$AUTHORITY_SEED"
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Owner('"$contract_address"')' --suri "$AUTHORITY_SEED"

  eval "$__resultvar='$contract_address'"
}

function deploy_and_instrument_game {

  local  __resultvar=$1
  local contract_name=$2
  local game_token=$3

  # --- UPLOAD CONTRACT CODE

  cd "$CONTRACTS_PATH/$contract_name"
  link_bytecode "$contract_name" 4465614444656144446561444465614444656144446561444465614444656144 "$ACCESS_CONTROL_PUBKEY"
  rm target/ink/"$contract_name".wasm
  node ../scripts/hex-to-wasm.js target/ink/"$contract_name".contract target/ink/"$contract_name".wasm

  local code_hash
  code_hash=$(cargo contract upload --url "$NODE" --suri "$AUTHORITY_SEED")
  code_hash=$(echo "$code_hash" | grep hash | tail -1 | cut -c 15-)

  # --- GRANT INIT PRIVILEGES ON THE CONTRACT CODE

  cd "$CONTRACTS_PATH"/access_control

  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Initializer('"$code_hash"')' --suri "$AUTHORITY_SEED"

  # --- CREATE AN INSTANCE OF THE CONTRACT

  cd "$CONTRACTS_PATH/$contract_name"

  local contract_address
  contract_address=$(cargo contract instantiate --url "$NODE" --constructor new --args "$game_token" "$LIFETIME" --suri "$AUTHORITY_SEED")
  contract_address=$(echo "$contract_address" | grep Contract | tail -1 | cut -c 15-)

  echo "$contract_name contract instance address: $contract_address"

  # --- GRANT PRIVILEDGES ON THE CONTRACT

  cd "$CONTRACTS_PATH"/access_control

  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Owner('"$contract_address"')' --suri "$AUTHORITY_SEED"
  cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Admin('"$contract_address"')' --suri "$AUTHORITY_SEED"

  # --- TRANSFER TOKENS TO THE CONTRACT

  cd "$CONTRACTS_PATH"/game_token

  cargo contract call --url "$NODE" --contract "$game_token" --message PSP22::transfer --args "$contract_address" "$GAME_BALANCE" "[0]" --suri "$AUTHORITY_SEED"

  # --- WHITELIST ACCOUNTS FOR PLAYING

  cd "$CONTRACTS_PATH/$contract_name"

  cargo contract call --url "$NODE" --contract "$contract_address" --message IButtonGame::bulk_allow --args "$WHITELIST" --suri "$AUTHORITY_SEED"

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

# mint this many tokens, 20% go to the future LP on DEX
TOTAL_BALANCE=1000
GAME_BALANCE=$(echo "0.8 * $TOTAL_BALANCE" | bc)

CONTRACTS_PATH=$(pwd)/contracts

# --- COMPILE CONTRACTS

cd "$CONTRACTS_PATH"/access_control
cargo contract build --release

cd "$CONTRACTS_PATH"/game_token
cargo contract build --release

cd "$CONTRACTS_PATH"/early_bird_special
cargo contract build --release

cd "$CONTRACTS_PATH"/back_to_the_future
cargo contract build --release

# --- DEPLOY ACCESS CONTROL CONTRACT

cd "$CONTRACTS_PATH"/access_control

CONTRACT=$(cargo contract instantiate --url "$NODE" --constructor new --suri "$AUTHORITY_SEED")
ACCESS_CONTROL=$(echo "$CONTRACT" | grep Contract | tail -1 | cut -c 15-)
ACCESS_CONTROL_PUBKEY=$(docker run --rm --entrypoint "/bin/sh" "${NODE_IMAGE}" -c "aleph-node key inspect $ACCESS_CONTROL" | grep hex | cut -c 23- | cut -c 3-)

echo "access control contract address: $ACCESS_CONTROL"
echo "access control contract public key \(hex\): $ACCESS_CONTROL_PUBKEY"

# --- UPLOAD TOKEN CONTRACT CODE

cd "$CONTRACTS_PATH"/game_token
# replace address placeholder with the on-chain address of the AccessControl contract
link_bytecode game_token 4465614444656144446561444465614444656144446561444465614444656144 "$ACCESS_CONTROL_PUBKEY"
# remove just in case
rm target/ink/game_token.wasm
# NOTE : here we go from hex to binary using a nodejs cli tool
# availiable from https://github.com/fbielejec/polkadot-cljs
node ../scripts/hex-to-wasm.js target/ink/game_token.contract target/ink/game_token.wasm

CODE_HASH=$(cargo contract upload --url "$NODE" --suri "$AUTHORITY_SEED")
GAME_TOKEN_CODE_HASH=$(echo "$CODE_HASH" | grep hash | tail -1 | cut -c 15-)

echo "button token code hash" "$GAME_TOKEN_CODE_HASH"

# --- GRANT INIT PRIVILEDGES ON THE TOKEN CONTRACT CODE

cd "$CONTRACTS_PATH"/access_control

# set the initializer of the token contract
cargo contract call --url "$NODE" --contract "$ACCESS_CONTROL" --message grant_role --args "$AUTHORITY" 'Initializer('"$GAME_TOKEN_CODE_HASH"')' --suri "$AUTHORITY_SEED"

#
# --- EARLY_BIRD_SPECIAL GAME
#

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE EARLY_BIRD_SPECIAL GAME

start=$( date +%s.%N )

instrument_game_token EARLY_BIRD_SPECIAL_TOKEN game_token Ubik UBI 0x4561726C79426972645370656369616C

# --- UPLOAD CODE AND CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME CONTRACT

deploy_and_instrument_game EARLY_BIRD_SPECIAL early_bird_special "$EARLY_BIRD_SPECIAL_TOKEN"

#
# --- BACK_TO_THE_FUTURE GAME
#

# --- CREATE AN INSTANCE OF THE TOKEN CONTRACT FOR THE BACK_TO_THE_FUTURE GAME

instrument_game_token BACK_TO_THE_FUTURE_TOKEN game_token Cyberiad CYB 0x4261636B546F546865467574757265

# --- UPLOAD CODE AND CREATE AN INSTANCE OF THE EARLY_BIRD_SPECIAL GAME CONTRACT

deploy_and_instrument_game BACK_TO_THE_FUTURE back_to_the_future "$BACK_TO_THE_FUTURE_TOKEN"

# spit adresses to a JSON file
cd "$CONTRACTS_PATH"

jq -n --arg early_bird_special "$EARLY_BIRD_SPECIAL" \
   --arg back_to_the_future "$BACK_TO_THE_FUTURE" \
   '{early_bird_special: $early_bird_special, back_to_the_future: $back_to_the_future}' > addresses.json

end=$( date +%s.%N )
echo "Time elapsed: $( echo "$end - $start" | bc -l )"

exit $?
