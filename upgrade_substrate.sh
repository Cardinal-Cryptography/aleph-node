#!/bin/bash

if [ -z "$1" ]
then
    echo "usage ./upgrade_substrate.sh [hash of a git commit in the substrate repository]"
    exit
fi

set -e

paths=(bin/node/Cargo.toml bin/runtime/Cargo.toml finality-aleph/Cargo.toml
    primitives/Cargo.toml pallet/Cargo.toml)

for path in ${paths[@]}; do
    echo upgrade $path
    sed -e 's/\(substrate.git.*rev = "\).*"/\1'$1'"/' < $path > x
    mv x $path
done

# cargo update
