#!/usr/bin/env bash
set -e

current_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
cd "${current_dir}/../.."

cargo build --bin fountain-encode --release --features encode
cargo build --bin fountain-decode --release --features decode

bash "${current_dir}/compile.wasm.sh"
