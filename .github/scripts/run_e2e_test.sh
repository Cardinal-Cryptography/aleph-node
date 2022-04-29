#!/bin/bash

set -e

while getopts t: opt
do
  case $opt in
    t) TEST_CASE=$OPTARG;;
    \?) echo "Invalid option: -$OPTARG";;
  esac
done

# source docker/env

docker run -v $(pwd)/docker/data:/data --network container:damian -e TEST_CASE -e NODE_URL=127.0.0.1:9943 -e RUST_LOG=info aleph-e2e-client:latest

exit $?
