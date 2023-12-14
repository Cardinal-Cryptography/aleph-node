#!/usr/bin/env bash

set -euo pipefail
source ./scripts/common.sh

# ------------------------ constants -------------------------------------------

export NODE_ID=5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH
CHAINSPEC_FILE="./liminal-benchmark-chainspec.json"

# ------------------------ argument parsing and usage --------------------------

function usage(){
  cat << EOF
Usage:
  $0
  --vk-storage
      Run benchmarks for the vk-storage pallet
  --chain-extension
      Run benchmarks for the baby liminal chain extension
EOF
  exit 0
}

VK_STORAGE=""
CHAIN_EXTENSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --vk-storage)
      VK_STORAGE="true"
      shift
      ;;
    --chain-extension)
      CHAIN_EXTENSION="true"
      shift
      ;;
    --help)
      usage
      shift
      ;;
    *)
      error "Unrecognized argument $1!"
      ;;
  esac
done

# ------------------------ cleaning --------------------------------------------
function cleanup() {
  rm -rf "${CHAINSPEC_FILE}"
}

function sigint_trap() {
  echo "Ctrl+C pressed, performing cleanup."
  cleanup
}

trap sigint_trap SIGINT
trap cleanup EXIT

# ------------------------ functions -------------------------------------------
function bootstrap() {
  cargo run --release -p aleph-node --features liminal-runtime-benchmarks -- bootstrap-chain \
    --base-path /tmp/ \
    --account-ids $NODE_ID \
    --sudo-account-id $NODE_ID \
    --chain-id liminalnet \
    --token-symbol LMNLZERO \
    --chain-name 'Aleph Zero LiminalNet' \
    > "${CHAINSPEC_FILE}"
}

function benchmark_pallet() {
  cargo run --release -p aleph-node --features liminal-runtime-benchmarks -- benchmark pallet \
      --chain="${CHAINSPEC_FILE}" \
      --pallet=pallet_vk_storage \
      --extrinsic='*' \
      --steps=20 \
      --repeat=5 \
      --template=.maintain/pallet-weight-template.hbs \
      --wasm-execution=compiled \
      --output=pallets/vk-storage/src/weights.rs
}

function benchmark_chain_extension() {
  cargo run --release -p aleph-node --features liminal-runtime-benchmarks -- benchmark pallet \
      --chain="${CHAINSPEC_FILE}" \
      --pallet=baby_liminal_extension \
      --extrinsic='*' \
      --steps=20 \
      --repeat=5 \
      --wasm-execution=compiled
}

# ------------------------ main ------------------------------------------------

if [[ -z "${VK_STORAGE}" && -z "${CHAIN_EXTENSION}" ]] ; then
  echo "No benchmarks selected, exiting."
fi

if [[ "${VK_STORAGE}" == "true" ]]; then
  bootstrap
  benchmark_pallet
fi

if [[ "${CHAIN_EXTENSION}" == "true" ]]; then
  bootstrap
  benchmark_chain_extension
fi

exit 0
