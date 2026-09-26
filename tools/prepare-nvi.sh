#!/bin/sh
# Unpack the nvi (Berkeley vi) release into the local scratch directory.
#
# Usage: sh tools/prepare-nvi.sh
#
# nvi is a freely redistributable rewrite of the historical ex/vi. It is not
# Vim. The untarred tree lives in _qemu/src/nvi-1.81.6, which is scratch space
# and is never committed. The upstream licence is copied into ports/nvi/ so the
# terms travel with the port even though the sources themselves do not.

set -e

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=1.81.6
SRC=${SBOS_NVI_SRC:-$ROOT/_qemu/src/nvi-$VERSION}
TARBALL=${SBOS_NVI_TARBALL:-$ROOT/_qemu/downloads/nvi_$VERSION.orig.tar.gz}
PORT=$ROOT/ports/nvi

die() {
    echo "prepare-nvi: $1" >&2
    exit 1
}

if [ ! -d "$SRC" ]; then
    [ -f "$TARBALL" ] || die "release tarball not found at $TARBALL
Download the nvi $VERSION source release into _qemu/downloads first."
    mkdir -p "$ROOT/_qemu/src"
    echo "prepare-nvi: unpacking $(basename "$TARBALL")"
    tar xzf "$TARBALL" -C "$ROOT/_qemu/src"
fi

mkdir -p "$PORT"
if [ -f "$SRC/LICENSE" ]; then
    cp "$SRC/LICENSE" "$PORT/LICENSE"
    echo "prepare-nvi: kept the upstream licence at ports/nvi/LICENSE"
else
    die "LICENSE is missing from $SRC; refusing to set up the port without it"
fi
