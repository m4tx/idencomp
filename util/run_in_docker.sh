#!/usr/bin/env bash

set -e
set -o allexport

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)
cd "$SCRIPT_DIR/.."

docker build -t idencomp-build .
docker run \
  -v "$(pwd)/out:/out" \
  idencomp-build \
  sh -c "$*"
