#!/usr/bin/env bash
set -e

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

cd "$(dirname "${BASH_SOURCE[0]}")/.."

try_build_rust_builder

${FOUNTAIN_DOCKER} run --rm \
    -v "$(pwd):/code" \
    "${FOUNTAIN_IMAGE_RUST_BUILDER}" /code/script/rust/compile.sh
