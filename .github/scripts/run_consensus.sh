#!/bin/bash

set -e

# change it when increasing number of node containers
export NODE_COUNT=5

mkdir -p docker/data/

echo "Generating accounts ids..."
declare -a account_ids
for node_index in $(seq 0 $((NODE_COUNT - 1))); do
  account_ids+=($(docker run -v $(pwd)/docker/data:/data --entrypoint "/bin/sh" -e node_index aleph-node:latest -c "aleph-node key inspect //$node_index | grep \"SS58 Address:\" | awk \"{print \\\$3;}\""))
done
# space separated ids
validator_ids="${account_ids[*]}"
# comma separated ids
validator_ids="${validator_ids//${IFS:0:1}/,}"

echo "Generate chainspec and keystores..."
docker run -v $(pwd)/docker/data:/data --entrypoint "/bin/sh" -e RUST_LOG=info -e validator_ids aleph-node:latest -c \
"aleph-node bootstrap-chain --base-path /data --account-ids $validator_ids  > /data/chainspec.json"

echo "Genereting bootnode peer id..."
bootnote_account=${account_ids[0]}
export BOOTNODE_PEER_ID=$(docker run -v $(pwd)/docker/data:/data --entrypoint "/bin/sh" -e bootnote_account -e RUST_LOG=info aleph-node:latest -c "aleph-node key inspect-node-key --file /data/$bootnote_account/p2p_secret")

echo "Running ${NODE_COUNT} containers..."
docker-compose -f docker/docker-compose.yml up -d

exit $?
