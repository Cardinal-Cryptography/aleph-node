# Tendermint LC pallet

## Run benchmarks

```rust
cargo build --release --features runtime-benchmarks

./target/release/aleph-node benchmark --chain docker/data/chainspec.json --extrinsic='*' --pallet=pallet-tendermint-light-client s --template=./.maintain/pallet-weight-template.hbs --output ./pallets/tendermint-light-client/src/weights.rs 
```
