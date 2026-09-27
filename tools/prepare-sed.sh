#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ARCHIVE="$ROOT/_qemu/downloads/gnu-tools/sed-4.10.tar.xz"
SOURCE="$ROOT/_qemu/src/sed-4.10"

if [ ! -f "$SOURCE/configure" ]; then
    [ -f "$ARCHIVE" ] || {
        echo "GNU sed source archive not found: $ARCHIVE" >&2
        exit 1
    }
    mkdir -p "$SOURCE"
    tar -xJf "$ARCHIVE" -C "$SOURCE" --strip-components=1
fi

grep -q "^PACKAGE_VERSION='4\\.10'" "$SOURCE/configure" || {
    echo "expected the GNU sed 4.10 source tree at $SOURCE" >&2
    exit 1
}

if ! grep -q '__sbos_program_short_name' "$SOURCE/lib/getprogname.c"; then
    (cd "$SOURCE" && patch --batch --forward -p1 <"$ROOT/tools/patches/sed-4.10-sbos.patch")
fi
if ! grep -q '__fwriting (fp)' "$SOURCE/lib/fwriting.c"; then
    (cd "$SOURCE" && patch --batch --forward -p1 <"$ROOT/tools/patches/sed-4.10-fwriting-sbos.patch")
fi

echo "GNU sed 4.10 source ready at $SOURCE"
