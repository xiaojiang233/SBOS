#!/bin/sh
# Build the official GNU Bash 5.3 release and patch level for SBOS.
set -e
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export SBOS_BASH_VERSION=5.3
exec sh "$ROOT/tools/build-bash.sh" "$@"
