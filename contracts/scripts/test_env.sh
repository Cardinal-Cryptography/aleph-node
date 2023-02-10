#!/bin/bash

set -euo pipefail

export CONTRACTS_PATH
CONTRACTS_PATH=$(pwd)/contracts

export EARLY_BIRD_SPECIAL
EARLY_BIRD_SPECIAL=$(jq --raw-output ".early_bird_special" < "$CONTRACTS_PATH"/addresses.json)
export THE_PRESSIAH_COMETH
THE_PRESSIAH_COMETH=$(jq --raw-output ".the_pressiah_cometh" < "$CONTRACTS_PATH"/addresses.json)
export BACK_TO_THE_FUTURE
BACK_TO_THE_FUTURE=$(jq --raw-output ".back_to_the_future" < "$CONTRACTS_PATH"/addresses.json)
export SIMPLE_DEX
SIMPLE_DEX=$(jq --raw-output ".simple_dex" < "$CONTRACTS_PATH"/addresses.json)
export WRAPPED_AZERO
WRAPPED_AZERO=$(jq --raw-output ".wrapped_azero" < "$CONTRACTS_PATH"/addresses.json)

export BUTTON_GAME_METADATA=$CONTRACTS_PATH/button/target/ink/metadata.json
export TICKET_TOKEN_METADATA=$CONTRACTS_PATH/ticket_token/target/ink/metadata.json
export REWARD_TOKEN_METADATA=$CONTRACTS_PATH/game_token/target/ink/metadata.json
export MARKETPLACE_METADATA=$CONTRACTS_PATH/marketplace/target/ink/metadata.json
export SIMPLE_DEX_METADATA=$CONTRACTS_PATH/simple_dex/target/ink/metadata.json
export WRAPPED_AZERO_METADATA=$CONTRACTS_PATH/wrapped_azero/target/ink/metadata.json
export RUST_LOG="aleph_e2e_client=info"
