#!/bin/sh
# GNU sed uses the same freestanding target driver and SBOS runtime as grep.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$ROOT/tools/grep-cc.sh" "$@"
