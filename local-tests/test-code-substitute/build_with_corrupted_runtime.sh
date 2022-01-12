set -e

TEST_DIR=local-tests/test-code-substitute
pushd ../../ > /dev/null

echo "Build node with old runtime"

cp $TEST_DIR/libs/runtime-lib-v6.rs bin/runtime/src/lib.rs
cargo build --release -p aleph-node
cp target/release/aleph-node $TEST_DIR/build/

echo "Build corrupted runtime"

cp $TEST_DIR/libs/pallet-corrupted-lib.rs pallet/src/lib.rs
cp $TEST_DIR/libs/runtime-lib-v7.rs bin/runtime/src/lib.rs

cargo build --release -p aleph-runtime
cp target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm $TEST_DIR/build/corrupted_runtime.wasm

echo "Build fixing runtime"

cp $TEST_DIR/libs/pallet-lib.rs pallet/src/lib.rs

cargo build --release -p aleph-runtime
cp target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm $TEST_DIR/build/fixing_runtime.wasm

echo "Build new runtime"

cp $TEST_DIR/libs/runtime-lib-v8.rs bin/runtime/src/lib.rs

cargo build --release -p aleph-runtime
cp target/release/wbuild/aleph-runtime/aleph_runtime.compact.wasm $TEST_DIR/build/new_runtime.wasm
