#!/bin/bash

set -e

export BOOTNODE_PEER_ID=$(./target/debug/aleph-node key inspect-node-key --file docker/data/5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/p2p_secret)

docker-compose -f docker/docker-compose.yml up -d

exit $?
