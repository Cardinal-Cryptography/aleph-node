#!/bin/bash

NODE_URL="${NODE_URL:-ws://localhost:9944}"
AUTHORITY="${AUTHORITY:-//Alice}"

cargo contract build --release
cargo contract upload --url "$NODE_URL" --suri "$AUTHORITY"

export ADDER

ADDER=$(
  cargo contract instantiate --url "$NODE_URL" --suri "$AUTHORITY" --skip-confirm --output-json \
    | jq -r ".contract"
)
echo "$ADDER"
