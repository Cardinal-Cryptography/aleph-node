#!/bin/bash

NODE=ws://127.0.0.1:9943
CONTRACTS_PATH=/home/filip/CloudStation/aleph/aleph-node/contracts
CLI_PATH=/home/filip/CloudStation/aleph/aleph-node/bin/cliain

function unquote() {
  echo $(echo $1 | xargs -n1 echo)
}

cd $CONTRACTS_PATH/button-token
# cargo +nightly contract build --release

cd $CLI_PATH

# deploy
DEPLOYED_CONTRACT=$(cargo run -- --seed '//Damian' --node $NODE contract-instantiate-with-code --gas-limit 100000000000 --wasm-path "$CONTRACTS_PATH/button-token/target/ink/button_token.wasm" --metadata-path "$CONTRACTS_PATH/button-token/target/ink/metadata.json" --args 1000)

echo "contract deployed: "  $DEPLOYED_CONTRACT

CONTRACT_ADDRESS=$(unquote $(echo $DEPLOYED_CONTRACT | jq '.contract'))
CODE_HASH=$(unquote $(echo $DEPLOYED_CONTRACT | jq '.code_hash'))
# CODE_HASH=0x9f52c78f0632ad35b69d3798c5f8a69a0171f6dedd760168cc99cc971688a4cf

echo "contract address: " $CONTRACT_ADDRESS

# terminate
# cargo run -- --seed '//Damian' --node $NODE contract-call --destination $CONTRACT_ADDRESS --metadata-path "$CONTRACTS_PATH/button-token/target/ink/metadata.json" --message "terminate"

# contract deployed:  {"contract":"5H2frv1go8vUjw2ArJyqvvrF2eAfiroGKZyRoWLHSWdtkP2V","code_hash":"0x9f52c78f0632ad35b69d3798c5f8a69a0171f6dedd760168cc99cc971688a4cf"}


# echo "code hash: " $CODE_HASH

# remove contract
# cargo run -- --seed '//Damian' --node $NODE contract-remove-code --code-hash $CODE_HASH

# wss://ws-smartnet.test.azero.dev

exit $?
