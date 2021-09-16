#!/bin/bash

function usage(){
  echo "Usage:
      ./run_nodes.sh [-v N_VALIDATORS] [-n N_NON_VALIDATORS] [-s] [ALEPH_NODE_ARG]...
  where 2 <= N_VALIDATORS <= N_VALIDATORS + N_NON_VALIDATORS <= 10
  (by default, N_VALIDATORS=4 and N_NON_VALIDATORS=0)"
}

N_VALIDATORS=4
N_NON_VALIDATORS=0
BUILD_ALEPH_NODE='true'

while getopts "v:n:s" flag
do
  case "${flag}" in
    v) N_VALIDATORS=${OPTARG};;
    n) N_NON_VALIDATORS=${OPTARG};;
    s) BUILD_ALEPH_NODE='false';;
    *)
      usage
      exit
      ;;
  esac
done

shift $((OPTIND-1))

killall -9 aleph-node

set -e

clear

if $BUILD_ALEPH_NODE ; then
  cargo build --release -p aleph-node
fi

account_ids=(
    "5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH"
    "5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o" \
    "5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9" \
    "5F4H97f7nQovyrbiq4ZetaaviNwThSVcFobcA5aGab6167dK" \
    "5DiDShBWa1fQx6gLzpf3SFBhMinCoyvHM1BWjPNsmXS8hkrW" \
    "5EFb84yH9tpcFuiKUcsmdoF7xeeY3ajG1ZLQimxQoFt9HMKR" \
    "5DZLHESsfGrJ5YzT3HuRPXsSNb589xQ4Unubh1mYLodzKdVY" \
    "5GHJzqvG6tXnngCpG7B12qjUvbo5e4e9z8Xjidk3CQZHxTPZ" \
    "5CUnSsgAyLND3bxxnfNhgWXSe9Wn676JzLpGLgyJv858qhoX" \
    "5CVKn7HAZW1Ky4r7Vkgsr7VEW88C2sHgUNDiwHY9Ct2hjU8q")
validator_ids=("${account_ids[@]::N_VALIDATORS}")
# space separated ids
validator_ids_string="${validator_ids[*]}"
# comma separated ids
validator_ids_string="${validator_ids_string//${IFS:0:1}/,}"


echo "Bootstrapping chain for nodes 0..$((N_VALIDATORS - 1))"
./target/release/aleph-node bootstrap-chain --base-path docker/data --chain-id dev --account-ids "$validator_ids_string" > docker/data/chainspec.json

for i in $(seq "$N_VALIDATORS" "$(( N_VALIDATORS + N_NON_VALIDATORS - 1 ))"); do
  echo "Bootstrapping node $i"
  account_id=${account_ids[$i]}
  ./target/release/aleph-node bootstrap-node --base-path docker/data --chain-id dev --account-id "$account_id"
done

for i in $(seq 0 "$(( N_VALIDATORS + N_NON_VALIDATORS - 1 ))"); do
  auth="node-$i"
  account_id=${account_ids[$i]}
  ./target/release/aleph-node purge-chain --base-path "docker/data/$account_id" --chain docker/data/chainspec.json -y
  ./target/release/aleph-node \
    --validator \
    --chain docker/data/chainspec.json \
    --base-path "docker/data/$account_id" \
    --name "$auth" \
    --rpc-port "$((9933 + i))" \
    --ws-port "$((9944 + i))" \
    --port "$((30334 + i))" \
    --execution Native \
    -lafa=debug \
    "$@" \
    2> "$auth.log" > /dev/null & \
done
