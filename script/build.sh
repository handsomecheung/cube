#!/usr/bin/env bash
set -e

cd "$(dirname "${BASH_SOURCE[0]}")/.."

image=cube-rust-builder

DOCKER="${DOCKER:-docker}"

if [[ "$(${DOCKER} images -q ${image} 2>/dev/null)" == "" ]]; then
    echo "not builder image, building it ..."
    # This build will take significantly longer due to vcpkg compiling OpenCV
    ${DOCKER} build -t ${image} .
fi

# VCPKGRS_DYNAMIC=0 forces static linking for vcpkg dependencies
# OPENCV_LINKAGE=static tells the opencv crate to link statically
${DOCKER} run --rm \
    -v "$(pwd):/code" \
    -e VCPKGRS_DYNAMIC=0 \
    -e OPENCV_LINKAGE=static \
    "${image}" ./script/compile.sh
