#!/bin/bash

# set -x
set -eo pipefail

TOML_FILE="Cargo.toml"

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

  if [[ "$p" =~ .*contracts.* ]]; then
     docker run \
      --network host \
      -v "$PWD:/code" \
      -u "$(id -u):$(id -g)" \
      --name ink_builder \
      --platform linux/amd64 \
      --rm public.ecr.aws/p6e8q1z1/ink-dev:2.0.0 cargo contract check

  elif [[ "$p" == "baby-liminal-extension" ]]; then
    # cargo clippy --release --no-default-features --features substrate \
      #  --target wasm32-unknown-unknown -- --no-deps -D warnings
    :
  elif [[ "$p" == "pallets/baby-liminal" ]]; then
    cargo test --features runtime-benchmarks
  else
    cargo clippy -- --no-deps -D warnings
  fi

  popd
done
