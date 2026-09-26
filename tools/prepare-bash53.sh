#!/bin/sh
# Prepare upstream GNU Bash 5.3 plus the official patch series in local scratch.
set -e

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SRC=${SBOS_BASH_SRC:-$ROOT/_qemu/src/bash-5.3}
TARBALL=${SBOS_BASH_TARBALL:-$ROOT/_qemu/downloads/bash-5.3.tar.gz}
PATCHES=$ROOT/ports/bash-5.3/patches

die() { echo "prepare-bash53: $1" >&2; exit 1; }

if [ ! -d "$SRC" ]; then
    [ -f "$TARBALL" ] || die "GNU Bash 5.3 source archive not found at $TARBALL"
    mkdir -p "$ROOT/_qemu/src"
    tar xzf "$TARBALL" -C "$ROOT/_qemu/src"
fi

[ -f "$SRC/configure" ] || die "not a GNU Bash 5.3 source tree: $SRC"
grep -q "bash 5.3-release" "$SRC/configure" || die "expected upstream Bash 5.3 source"
[ -d "$PATCHES" ] || die "official patch series is missing: $PATCHES"

cd "$SRC"
level=$(sed -n 's/^#define PATCHLEVEL[[:space:]][[:space:]]*\([0-9][0-9]*\).*/\1/p' patchlevel.h)
[ -n "$level" ] || die "could not read patch level from $SRC/patchlevel.h"
for patch_file in "$PATCHES"/bash53-*; do
    [ -f "$patch_file" ] || die "no official Bash 5.3 patches found"
    number=$(basename "$patch_file" | sed 's/^bash53-//')
    number=$(printf '%s' "$number" | sed 's/^0*//')
    [ -n "$number" ] || number=0
    [ "$number" -le "$level" ] && continue
    [ "$number" -eq $((level + 1)) ] || die "patch sequence has a gap before $patch_file (source level $level)"
    patch -p0 --forward <"$patch_file" >/dev/null || die "$(basename "$patch_file") does not apply cleanly"
    level=$number
    echo "prepare-bash53: applied $(basename "$patch_file")"
done

for patch_file in "$PATCHES"/sbos-*.patch; do
    [ -f "$patch_file" ] || continue
    if patch -p1 --forward --dry-run <"$patch_file" >/dev/null 2>&1; then
        patch -p1 --forward <"$patch_file" >/dev/null
        echo "prepare-bash53: applied $(basename "$patch_file")"
    elif patch -p1 --reverse --dry-run <"$patch_file" >/dev/null 2>&1; then
        echo "prepare-bash53: $(basename "$patch_file") already applied"
    else
        die "$(basename "$patch_file") does not apply cleanly"
    fi
done

cp "$SRC/COPYING" "$ROOT/ports/bash-5.3/COPYING"
