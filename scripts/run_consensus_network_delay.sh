#!/bin/bash

set -euo pipefail

NETWORK_DELAY=${NETWORK_DELAY:-500}
BUILD_IMAGE=${BUILD_IMAGE:-true}
NODES=${NODES:-"Node0:Node1:Node2:Node3:Node4"}

function build_test_image() {
    docker build -t aleph-node:network_tests -f docker/Dockerfile.network_tests .
}

function set_network_delay() {
    local node=$1
    local delay=$2

    log "setting network delay for node $node"
    docker exec $node tc qdisc add dev eth1 root netem delay ${delay}ms
}

function log() {
    echo "$1" 1>&2
}

function into_array() {
    result=()
    local tmp=$IFS
    IFS=:
    for e in $1; do
        result+=($e)
    done
    IFS=$tmp
}

into_array $NODES
NODES=(${result[@]})

if [[ "$BUILD_IMAGE" = true ]]; then
    log "building docker image for network tests"
    build_test_image
fi

log "starting network"
OVERRIDE_DOCKER_COMPOSE=./docker/docker-compose.network_tests.yml DOCKER_COMPOSE=./docker/docker-compose.bridged.yml ./.github/scripts/run_consensus.sh 1>&2
log "network started"

log "setting network delay"
for node in ${NODES[@]}; do
    set_network_delay $node $NETWORK_DELAY
done

log "done"
