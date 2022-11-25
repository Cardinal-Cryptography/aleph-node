#!/bin/bash

set -euo pipefail

E2E_PATH=$(pwd)/e2e-tests
CONTRACTS_PATH=$(pwd)/contracts
EARLY_BIRD_SPECIAL=$(jq --raw-output ".early_bird_special" < "$CONTRACTS_PATH"/addresses.json)
THE_PRESSIAH_COMETH=$(jq --raw-output ".the_pressiah_cometh" < "$CONTRACTS_PATH"/addresses.json)
BACK_TO_THE_FUTURE=$(jq --raw-output ".back_to_the_future" < "$CONTRACTS_PATH"/addresses.json)
SIMPLE_DEX=$(jq --raw-output ".simple_dex" < "$CONTRACTS_PATH"/addresses.json)
WRAPPED_AZERO=$(jq --raw-output ".wrapped_azero" < "$CONTRACTS_PATH"/addresses.json)

pushd "$E2E_PATH"

RUST_LOG="aleph_e2e_client=info" cargo run --release -- \
  --test-cases wrapped_azero \
  --test-cases simple_dex \
  --test-cases marketplace \
  --test-cases button_game_reset \
  --test-cases early_bird_special \
  --test-cases the_pressiah_cometh \
  --test-cases back_to_the_future \
  --early-bird-special "$EARLY_BIRD_SPECIAL" \
  --the-pressiah-cometh "$THE_PRESSIAH_COMETH" \
  --back-to-the-future "$BACK_TO_THE_FUTURE" \
  --simple-dex "$SIMPLE_DEX" \
  --wrapped-azero "$WRAPPED_AZERO" \
  --button-game-metadata ../contracts/button/target/ink/metadata.json \
  --ticket-token-metadata ../contracts/ticket_token/target/ink/metadata.json \
  --reward-token-metadata ../contracts/game_token/target/ink/metadata.json \
  --marketplace-metadata ../contracts/marketplace/target/ink/metadata.json \
  --simple-dex-metadata ../contracts/simple_dex/target/ink/metadata.json \
  --wrapped-azero-metadata ../contracts/wrapped_azero/target/ink/metadata.json

exit $?
