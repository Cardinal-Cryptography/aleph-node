set -e

# `packages` should reflect `exclude` section from root `Cargo.toml`.
packages=(
  "flooder"
  "e2e-tests"
  "aleph-client"
  "fork-off"
  "benches/payout-stakers"
  "bin/cliain"
)

for p in ${packages[@]}
do
  echo "Compiling package $p..."
  pushd "$p"
  cargo clippy --all-targets --all-features -- --no-deps -D warnings
  popd
done
