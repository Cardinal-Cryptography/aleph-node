#!/bin/bash

set -e

docker run -v $(pwd)/docker/data:/data --network container:damian -e NODE_URL=127.0.0.1:9943 -e DEBUG_STORAGE=debug-storage aleph-e2e-client:latest

exit $?
