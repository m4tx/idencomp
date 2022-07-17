#!/usr/bin/env bash

set -e

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)

# shellcheck disable=SC2088
"$SCRIPT_DIR/util/run_in_docker.sh" \
  cargo bench
