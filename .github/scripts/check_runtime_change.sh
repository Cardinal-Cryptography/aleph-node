#!/bin/bash

set -eu

if [ $# -ne 3 ]; then
  echo "Expected 3 arguments (paths to repositories and output name)"
  exit 2
fi

grep "spec_version:" "${$1%/}/bin/runtime/src/lib.rs" | grep -o '[0-9]*' > old.version
grep "spec_version:" "${$2%/}/bin/runtime/src/lib.rs" | grep -o '[0-9]*' > new.version

diff old.version new.version

echo "::set-output name=$3::$?"
