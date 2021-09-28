#!/usr/bin/env bash
set -euo pipefail

aleph-e2e-client --base-path $BASE_PATH --account-ids $DAMIAN $TOMASZ $ZBYSZKO $HANSU --sudo-account-id $DAMIAN

echo "Done!"
