#!/usr/bin/env bash
set -e

DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

export FOUNTAIN_DOCKER="${DOCKER:-docker}"
export FOUNTAIN_IMAGE_RUST_BUILDER=fountain-rust-builder
export FOUNTAIN_CONTAINER_CAMERA_WEB=fountain-decode-camera-web

try_build_rust_builder() {
    if [[ "$(${FOUNTAIN_DOCKER} images -q ${FOUNTAIN_IMAGE_RUST_BUILDER} 2>/dev/null)" == "" ]]; then
        echo "not builder image, building it ..."
        cd "${DIR}/.."
        ${FOUNTAIN_DOCKER} build -t ${FOUNTAIN_IMAGE_RUST_BUILDER} .
        cd -
    fi
}
