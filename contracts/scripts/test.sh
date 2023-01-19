#!/bin/bash

set -euo pipefail

E2E_PATH=$(pwd)/e2e-tests
CONTRACTS_PATH=$(pwd)/contracts

pushd "$E2E_PATH"

EARLY_BIRD_SPECIAL=$(jq --raw-output ".early_bird_special" < "$CONTRACTS_PATH"/addresses.json) \
  THE_PRESSIAH_COMETH=$(jq --raw-output ".the_pressiah_cometh" < "$CONTRACTS_PATH"/addresses.json) \
  BACK_TO_THE_FUTURE=$(jq --raw-output ".back_to_the_future" < "$CONTRACTS_PATH"/addresses.json) \
  SIMPLE_DEX=$(jq --raw-output ".simple_dex" < "$CONTRACTS_PATH"/addresses.json) \
  WRAPPED_AZERO=$(jq --raw-output ".wrapped_azero" < "$CONTRACTS_PATH"/addresses.json) \
  BUTTON_GAME_METADATA=$CONTRACTS_PATH/button/target/ink/metadata.json \
  TICKET_TOKEN_METADATA=$CONTRACTS_PATH/ticket_token/target/ink/metadata.json \
  REWARD_TOKEN_METADATA=$CONTRACTS_PATH/game_token/target/ink/metadata.json \
  MARKETPLACE_METADATA=$CONTRACTS_PATH/marketplace/target/ink/metadata.json \
  SIMPLE_DEX_METADATA=$CONTRACTS_PATH/simple_dex/target/ink/metadata.json \
  WRAPPED_AZERO_METADATA=$CONTRACTS_PATH/wrapped_azero/target/ink/metadata.json \
  RUST_LOG="aleph_e2e_client=info" \
  cargo test button -- --test-threads 1 --nocapture

exit $?
