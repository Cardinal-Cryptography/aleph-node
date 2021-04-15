#!/bin/bash

set -e

cargo +nightly clippy --all-targets --all-features
cargo +nightly fmt --all
cargo test --lib
