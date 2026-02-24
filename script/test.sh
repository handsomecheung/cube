#!/usr/bin/env bash
set -e

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

cd "$(dirname "${BASH_SOURCE[0]}")/.."

${FOUNTAIN_DOCKER} run --rm \
    -v "$(pwd):/code" \
    "${FOUNTAIN_IMAGE_RUST_BUILDER}" /code/script/rust/test.sh
