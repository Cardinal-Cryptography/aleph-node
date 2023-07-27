# --- FUNCTIONS

function run_ink_dev() {
  docker start ink_dev || docker run \
                                 --network host \
                                 -v "${CONTRACTS_PATH}:/code" \
                                 -v ~/.cargo/git:/usr/local/cargo/git \
                                 -v ~/.cargo/registry:/usr/local/cargo/registry \
                                 -u "$(id -u):$(id -g)" \
                                 --name ink_dev \
                                 --platform linux/amd64 \
                                 --detach \
                                 --rm $INK_DEV_IMAGE sleep 1d
}

function cargo_contract() {
  contract_dir=$(basename "${PWD}")
  docker exec \
         -u "$(id -u):$(id -g)" \
         -w "/code/$contract_dir" \
         -e RUST_LOG=info \
         ink_dev cargo contract "$@"
}

function get_address {
  local contract_name=$1
  cat $ADDRESSES_FILE | jq --raw-output ".$contract_name"
}

# defaults to wrapping and transferring 1K wA0 to the DEX
# value can be overriden with a first argument to the function
function add_liquidity() {
  local value="${1:-1000000000000000}"
  local wrapped_azero=$(get_address wrapped_azero)
  local dex=$(get_address simple_dex)

  cd "$CONTRACTS_PATH"/wrapped_azero
  cargo_contract call --url "$NODE" --contract "$wrapped_azero" --message wrap --value $value --suri "$AUTHORITY_SEED" --skip-confirm
  cargo_contract call --url "$NODE" --contract "$wrapped_azero" --message PSP22::transfer --args $dex $value "[0]" --suri "$AUTHORITY_SEED" --skip-confirm
}
