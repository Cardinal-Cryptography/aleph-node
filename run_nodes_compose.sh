#!/bin/bash

# USAGE :
# 
# ./run_node_compose -b true
# ARGUMENTS :
# -b false (default) | true : whether to build the binary and docker image

# args

while getopts b:p:t: flag
do
  case "${flag}" in
    b) BUILD=${OPTARG};;
  esac
done

# defaults

if [ -z ${BUILD+x} ];
then
  BUILD=false
fi

if [ $BUILD = true ]
then
  cargo build --release
  docker build --tag aleph-node:latest -f docker/Dockerfile .
fi

# remove old keys
rm -rf docker/data/Damian/chains/a0tnet1/keystore/ docker/data/Tomasz/chains/a0tnet1/keystore/ docker/data/Zbyszko/chains/a0tnet1/keystore/ docker/data/Hansu/chains/a0tnet1/keystore/

# NOTE : remove this step after keys are moved to a separate JSON file
# ensure these are in tmp/
cp docker/data/n_members /tmp/n_members
cp docker/data/authorities_keys /tmp/authorities_keys

# generate new keys
./target/release/aleph-node dev-keys --base-path docker/data --chain testnet1 --key-types aura alp0

# build chainspec
./target/release/aleph-node build-spec --disable-default-bootnode  --chain testnet1 > docker/data/chainspec.json

# launch consensus (you may need to change bootnode peer id!)
# docker-compose -f docker/docker-compose.yml up 
