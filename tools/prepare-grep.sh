#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ARCHIVE="$ROOT/_qemu/downloads/gnu-tools/grep-3.12.tar.xz"
SOURCE="$ROOT/_qemu/src/grep-3.12"

if [ ! -f "$SOURCE/configure" ]; then
    [ -f "$ARCHIVE" ] || {
        echo "GNU grep source archive not found: $ARCHIVE" >&2
        exit 1
    }
    mkdir -p "$SOURCE"
    tar -xJf "$ARCHIVE" -C "$SOURCE" --strip-components=1
fi

grep -q "^PACKAGE_VERSION='3\\.12'" "$SOURCE/configure" || {
    echo "expected the GNU grep 3.12 source tree at $SOURCE" >&2
    exit 1
}

if ! grep -q '__sbos_program_short_name' "$SOURCE/lib/getprogname.c"; then
    (cd "$SOURCE" && patch --batch --forward -p1 <"$ROOT/tools/patches/grep-3.12-sbos.patch")
fi

echo "GNU grep 3.12 source ready at $SOURCE"
