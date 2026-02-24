#!/usr/bin/env bash
set -e

cd "$(dirname "${BASH_SOURCE[0]}")/../.."

cargo build --target wasm32-unknown-unknown --release --no-default-features --features wasm
wasm-bindgen target/wasm32-unknown-unknown/release/fountain.wasm --out-dir www/pkg --target web
