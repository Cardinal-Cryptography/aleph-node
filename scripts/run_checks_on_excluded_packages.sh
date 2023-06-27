#!/bin/bash

# set -x
set -eo pipefail

TOML_FILE="Cargo.toml"
RUST_TOOLCHAIN=nightly-2022-10-30-x86_64-unknown-linux-gnu
RUST_CONTRACTS_TOOLCHAIN=nightly-2023-01-10-x86_64-unknown-linux-gnu

# Read the TOML file and extract the `exclude` entries
packages=$(awk -F ' *= *' '/^exclude *= *\[/ {found=1} found && /^\]$/ {found=0} found' "$TOML_FILE")

packages="$(echo ${packages} | sed 's/[][,]/ /g' | sed 's/\s\+/\n/g' | sed '/^$/d')"

# Remove leading and trailing whitespace, and quotes from the entries
packages=$(echo "$packages" | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e 's/^"//' -e 's/"$//')

packages="${packages//'%0A'/$'\n'}"

# Remove the key
packages=${packages:10}

for p in ${packages[@]}; do

  echo "Checking package $p ..."
  pushd "$p"

  if [[ $p =~ .*contracts.* ]] && [[ $p != "contracts/poseidon_host_bench" ]]; then
    cargo +${RUST_CONTRACTS_TOOLCHAIN} contract check
  elif [ $p = "baby-liminal-extension" ] || [ $p = "contracts/poseidon_host_bench" ]; then
    # cargo clippy --release --no-default-features --features substrate \
      #  --target wasm32-unknown-unknown -- --no-deps -D warnings
    :
  elif [ $p = "pallets/baby-liminal" ]; then
    cargo +${RUST_TOOLCHAIN} test --features runtime-benchmarks
  else
    cargo +${RUST_TOOLCHAIN} clippy -- --no-deps -D warnings
  fi

  cargo fmt --all --check
  popd

done
