fmt:
	cargo fmt --all

clippy:
	WASM_BUILD_WORKSPACE_HINT=${PWD} CARGO_TARGET_DIR=/tmp/aleph-node/target/ cargo clippy --all-targets -- --no-deps -D warnings

watch:
	cargo watch -s 'WASM_BUILD_WORKSPACE_HINT=${PWD} CARGO_TARGET_DIR=/tmp/aleph-node/target/ cargo clippy' -c

release:
	WASM_BUILD_WORKSPACE_HINT=${PWD} CARGO_TARGET_DIR=/tmp/aleph-node/target/ cargo build --release -p aleph-node --features "short_session enable_treasury_proposals" && cp /tmp/aleph-node/target/release/aleph-node target/release
