#!/bin/bash

./aleph-node --validator \
 --chain $CHAIN_NAME \
 --base-path $BASE_PATH \
 --name $NODE_NAME \
 --node-key-file $NODE_KEY_PATH \
 --rpc-port 9933 \
 --ws-port 9944 \
 --port 30334 \
 --rpc-cors all \
 --rpc-methods Safe \
 --execution Native \
 --no-prometheus \
 --no-telemetry \
 --reserved-only \
 --reserved-nodes $RESERVED_NODES
