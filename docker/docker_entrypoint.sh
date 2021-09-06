set -euo pipefail

# script env variables
PURGE_BEFORE_START=${PURGE_BEFORE_START:-}

# aleph_node cli options to env variables
CHAIN_NAME=${CHAIN_NAME:?'Chain name should be specified'}
NAME=${NAME:?'Name should be specified'}
BASE_PATH=${BASE_PATH:?'Base path should be specified'}
RPC_PORT=${RPC_PORT:-9933}
WS_PORT=${WS_PORT:-9943}
PORT=${PORT:-30333}

if [[ "true" == "$PURGE_BEFORE_START" ]]; then
  echo "Purging chain (${CHAIN_NAME}) at path ${BASE_PATH}"
  ./aleph-node purge-chain --base-path "${BASE_PATH}" --chain "${CHAIN_NAME}" -y
fi

ARGS=(
  --validator
  --execution Native
  --name "${NAME}"
  --base-path "${BASE_PATH}"
  --chain "${CHAIN_NAME}"
  --node-key-file "${NODE_KEY_PATH}"
  --rpc-port "${RPC_PORT}" --ws-port "${WS_PORT}" --port "${PORT}"
  --rpc-cors all --rpc-methods Safe # TODO: should we allow to specify them?
  --no-prometheus --no-telemetry # Currently not using. plan to start as soon as capacity is available
  --no-mdns
)

## No additional params are passed. Lets use default env variables
#if [ "$#" -ne 1 ]; then
#  ARGS=(
#  )
#fi

if [[ -n "${BOOT_NODES:-}" ]]; then
  ARGS+=(--reserved-nodes "${BOOT_NODES}")
fi

if [[ -n "${RESERVED_NODES:-}" ]]; then
  ARGS+=(--reserved-nodes "${RESERVED_NODES}")
fi

if [[ -n "${RESERVED_ONLY:-}" ]]; then
  ARGS+=(--reserved-only)
fi

if [[ -n "${FLAG_LAFA:-}" ]]; then
  ARGS+=(-lafa=debug)
fi

if [[ -n "${FLAG_L_ALEPH_BFT:-}" ]]; then
  ARGS+=(-lAlephBFT=debug)
fi

./aleph-node "${ARGS[@]}"
