#!/bin/bash

set -eu

pushd local-tests/

if [ ! -f "$BINARY" ]; then
  echo "Binary $BINARY does not exist."
  exit 1
fi

echo 'Preparing environment'
chmod +x $BINARY

pip install -r requirements.txt

echo 'Running test'
./test_catch_up.py

popd
