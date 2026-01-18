#!/usr/bin/env bash
set -e

cd "$(dirname "${BASH_SOURCE[0]}")/.."

DOCKER="${DOCKER:-docker}"

name=cube-decode-camera-web
${DOCKER} stop ${name} 2>/dev/null || true
${DOCKER} rm ${name} 2>/dev/null || true
${DOCKER} run --rm -d \
    --name ${name} \
    -p 37290:80 \
    -v "$(pwd)/script/nginx.decode-camera-web.conf:/etc/nginx/nginx.conf" \
    -v "$(pwd)/www:/cube/www" \
    nginx:latest
