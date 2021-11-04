#!/bin/bash

set -e

source docker/env

cd tests/e2e

cargo run -- --base-path docker/data --account-ids $DAMIAN $TOMASZ $ZBYSZKO $HANSU --sudo-account-id $DAMIAN

exit $?
