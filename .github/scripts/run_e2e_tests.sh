#!/bin/bash

set -e

source docker/env

docker run -v $(pwd)/docker/data:/data --network container:damian -e DAMIAN -e TOMASZ -e ZBYSZKO -e HANSU -e BASE_PATH=/data -e RUST_LOG=info aleph-e2e-client:latest

exit $?
