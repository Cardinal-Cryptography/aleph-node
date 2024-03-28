#!/usr/bin/env bash

# This file is part of Substrate.
# Copyright (C) Parity Technologies (UK) Ltd.
# SPDX-License-Identifier: Apache-2.0
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
# http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# This script has three parts which all use the Substrate runtime:
# - Pallet benchmarking to update the pallet weights
# - Overhead benchmarking for the Extrinsic and Block weights
# - Machine benchmarking
#
# Should be run on a reference machine to gain accurate benchmarks
# current reference machine: https://github.com/paritytech/substrate/pull/5848

#!/usr/bin/env bash

set -euo pipefail
source ./scripts/common.sh

# ------------------------ constants -------------------------------------------

export NODE_ID=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
CHAINSPEC_FILE="./benchmark-chainspec.json"

# ------------------------ argument parsing and usage --------------------------

function usage(){
  cat << EOF
Usage:
  $0
  --feature-control
      Run benchmarks for the feature-control pallet
  --vk-storage
      Run benchmarks for the vk-storage pallet
  --chain-extension
      Run benchmarks for the baby liminal chain extension
EOF
  exit 0
}

VK_STORAGE=""
FEATURE_CONTROL=""
CHAIN_EXTENSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --feature-control)
      FEATURE_CONTROL="true"
      shift
      ;;
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
#function cleanup() {
#   rm -rf "${CHAINSPEC_FILE}"
#}
#
#function sigint_trap() {
#  echo "Ctrl+C pressed, performing cleanup."
#  cleanup
#}
#
#trap sigint_trap SIGINT
#trap cleanup EXIT

# ------------------------ functions -------------------------------------------
function bootstrap() {
  cargo run --profile production -p aleph-node --features runtime-benchmarks -- bootstrap-chain \
    --base-path /tmp/ \
    --account-ids $NODE_ID,5F4H97f7nQovyrbiq4ZetaaviNwThSVcFobcA5aGab6167dK,5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9,5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o \
    --sudo-account-id $NODE_ID \
    --chain-id benchmarknet \
    --token-symbol BZERO \
    --rich-account-ids $NODE_ID \
    --chain-name 'Aleph Zero BenchmarkNet' \
    > "${CHAINSPEC_FILE}"
}


bootstrap

# The executable to use.
NODE=./target/production/aleph-node


# Load all pallet names in an array.
ALL_PALLETS=($(
  $NODE benchmark pallet --list --chain="${CHAINSPEC_FILE}"  |\
                                                                                       tail -n+2 |\
                                                                                       cut -d',' -f1 |\
                                                                                       sort |\
                                                                                       uniq

))

# Filter out the excluded pallets by concatenating the arrays and discarding duplicates.
PALLETS=($({ printf '%s\n' "${ALL_PALLETS[@]}"; } | sort | uniq -u))
echo "[+] Benchmarking ${#PALLETS[@]} Substrate pallets by excluding  from ${#ALL_PALLETS[@]}."

# Define the error file.
ERR_FILE="benchmarking_errors.txt"
# Delete the error file before each run.
rm -f $ERR_FILE

# Benchmark each pallet.
for PALLET in "${PALLETS[@]}"; do
  FOLDER="$(echo "${PALLET#*_}" | tr '_' '-')";
  WEIGHT_FOLDER="./weights/${FOLDER}"
  WEIGHT_FILE="./weights/${FOLDER}/weights.rs"
  mkdir -p $WEIGHT_FOLDER
  echo "[+] Benchmarking $PALLET with weight file $WEIGHT_FILE";

  OUTPUT=$(
    $NODE benchmark pallet \
    --chain="${CHAINSPEC_FILE}" \
    --steps=50 \
    --repeat=20 \
    --pallet="$PALLET" \
    --extrinsic="*" \
    --wasm-execution=compiled \
    --heap-pages=4096 \
    --output="$WEIGHT_FILE" \
    --template=./.maintain/pallet-weight-template.hbs
  )
  if [ $? -ne 0 ]; then
    echo "$OUTPUT" >> "$ERR_FILE"
    echo "[-] Failed to benchmark $PALLET. Error written to $ERR_FILE; continuing..."
  fi
done

mkdir -p "./weights/support"
# Update the block and extrinsic overhead weights.
echo "[+] Benchmarking block and extrinsic overheads..."
OUTPUT=$($NODE benchmark overhead \
  --chain=${CHAINSPEC_FILE} \
  --wasm-execution=compiled \
  --weight-path="./weights/support" \
  --warmup=10 \
  --repeat=100
  )
if [ $? -ne 0 ]; then
  echo "$OUTPUT" >> "$ERR_FILE"
  echo "[-] Failed to benchmark the block and extrinsic overheads. Error written to $ERR_FILE; continuing..."
fi

echo "[+] Benchmarking the machine..."
OUTPUT=$(
  $NODE benchmark machine --chain="${CHAINSPEC_FILE}"
)
if [ $? -ne 0 ]; then
  # Do not write the error to the error file since it is not a benchmarking error.
  echo "[-] Failed the machine benchmark:\n$OUTPUT"
fi

# Check if the error file exists.
if [ -f "$ERR_FILE" ]; then
  echo "[-] Some benchmarks failed. See: $ERR_FILE"
  exit 1
else
  echo "[+] All benchmarks passed."
  exit 0
fi
