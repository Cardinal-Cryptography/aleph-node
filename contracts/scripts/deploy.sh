#!/bin/bash

NODE=ws://127.0.0.1:9943
CONTRACTS_PATH=/home/filip/CloudStation/aleph/aleph-node/contracts
CLI_PATH=/home/filip/CloudStation/aleph/aleph-node/bin/cliain

function unquote() {
  echo $(echo $1 | xargs -n1 echo)
}


cd $CONTRACTS_PATH/button-token
# cargo +nightly contract build --release

# DEPLOYED_CONTRACT=$(cargo contract instantiate --url $NODE --constructor new --args 1000 --suri //Alice)
# DEPLOYED_CONTRACT=$(cat out)

# BUTTON_TOKEN=$(echo "$DEPLOYED_CONTRACT" | grep Contract | tail -1 | cut -c 15-)
BUTTON_TOKEN=5GmasQa5QBEPDYNpQye7Ht1pYmeBUceJQvwXZtCJUa3PS4mW
# CODE_HASH=$(echo "$DEPLOYED_CONTRACT" | grep "Code hash" | tail -1 | cut -c 15-_

# echo "code hash: " $CODE_HASH            
echo "contract address: " $BUTTON_TOKEN

cd $CONTRACTS_PATH/yellow-button
cargo +nightly contract build --release

# cargo contract call --url $NODE --contract 5FWkHZFoqSDPESUfws3nJM3QqdwCL4QENggGDSDeBM1VNHMc --message terminate --suri //Alice

exit $?
