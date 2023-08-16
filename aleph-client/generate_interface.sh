#!/bin/bash
cd "${0%/*}/.." || exit

function generate {
  extra_features="$1"
  output_file="$2"
  mkdir -p /tmp/aleph_client_codegen
    cargo build --release -p aleph-node --features "short_session enable_treasury_proposals $extra_features"
    ./scripts/run_nodes.sh -p /tmp/aleph_client_codegen -b false
    echo "Waiting till local network boots up..."
    sleep 5
    subxt codegen --derive Clone --derive Debug --derive PartialEq --derive Eq \
      | sed 's/::[ ]*subxt[ ]*::[ ]*utils[ ]*::[ ]*AccountId32/::subxt::utils::Static<::subxt::ext::sp_core::crypto::AccountId32>/g' \
      | rustfmt --edition=2021 > "$output_file"
    killall aleph-node
}

generate '' './aleph-client/src/aleph_zero.rs' || exit
generate 'liminal' 'aleph-client/src/aleph_zero_liminal.rs'
