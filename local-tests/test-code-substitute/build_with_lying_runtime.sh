set -e

TEST_DIR=local-tests/test-code-substitute
pushd ../../ > /dev/null

echo "Build node with old runtime"

cp $TEST_DIR/libs/runtime-lib-v6.rs bin/runtime/src/lib.rs
cargo build --release -p aleph-node
cp target/release/aleph-node $TEST_DIR/build/

echo "Build on-chain runtime"

cp $TEST_DIR/libs/runtime-lib-v7.rs bin/runtime/src/lib.rs

cargo build --release -p aleph-runtime
cp target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm $TEST_DIR/build/on_chain_runtime.wasm

echo "Build lying runtime"

cp $TEST_DIR/libs/runtime-lib-v7-fee.rs bin/runtime/src/lib.rs

cargo build --release -p aleph-runtime
cp target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm $TEST_DIR/build/lying_runtime.wasm
