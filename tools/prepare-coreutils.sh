#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SOURCE=${1:-"$ROOT/_qemu/src/coreutils-9.12/coreutils-9.12"}
PATCH_FILE="$ROOT/tools/patches/coreutils-9.12-sbos.patch"
MODULE_PATCH_FILE="$ROOT/tools/patches/coreutils-9.12-mountlist.patch"

if [ ! -f "$SOURCE/lib/freadahead.c" ] || [ ! -f "$SOURCE/lib/freading.c" ] \
   || [ ! -f "$SOURCE/lib/fseterr.c" ] || [ ! -f "$SOURCE/lib/fsync.c" ] \
   || [ ! -f "$SOURCE/lib/gnulib.mk" ] || [ ! -f "$SOURCE/Makefile.in" ] \
   || [ ! -f "$SOURCE/lib/getdtablesize.c" ] || [ ! -f "$SOURCE/lib/getprogname.c" ] \
   || [ ! -f "$SOURCE/lib/link.c" ] || [ ! -f "$SOURCE/lib/mountlist.h" ] \
   || [ ! -f "$SOURCE/lib/nanosleep.c" ] || [ ! -f "$SOURCE/lib/renameatu.c" ] \
   || [ ! -f "$SOURCE/lib/strtod.c" ] || [ ! -f "$SOURCE/lib/unistd.in.h" ]; then
    echo "Coreutils 9.12 source tree not found: $SOURCE" >&2
    exit 1
fi

if grep -q '__SBOS__.*opaque stdio object' "$SOURCE/lib/freadahead.c" \
   && grep -q '__SBOS__.*opaque stdio object' "$SOURCE/lib/freading.c" \
   && grep -q '__SBOS__.*opaque stdio object' "$SOURCE/lib/fseterr.c" \
   && grep -q 'int rpl_fsync (int fd)' "$SOURCE/lib/fsync.c" \
   && grep -q 'int rpl_getdtablesize (void)' "$SOURCE/lib/getdtablesize.c" \
   && grep -q '__sbos_program_short_name' "$SOURCE/lib/getprogname.c" \
   && grep -q 'return link (file1, file2);' "$SOURCE/lib/link.c" \
   && grep -q 'return nanosleep (requested_delay, remaining_delay);' "$SOURCE/lib/nanosleep.c" \
   && grep -q 'no-renameat fallback also checks trailing slashes' "$SOURCE/lib/renameatu.c" \
   && grep -q 'SBOS libc does not yet provide strtod/strtof/strtold' "$SOURCE/lib/strtod.c" \
   && grep -q 'SBOS uses fixed 4 KiB virtual-memory pages' "$SOURCE/lib/unistd.in.h" \
   && grep -q 'SBOS omits mount-list-dependent commands' "$SOURCE/lib/gnulib.mk" \
   && grep -q 'SBOS does not yet expose the POSIX spawn API' "$SOURCE/lib/gnulib.mk" \
   && grep -q 'SBOS has no Ring 3 socket/name-service API' "$SOURCE/lib/gnulib.mk" \
   && grep -q 'SBOS omits name-service substitutes' "$SOURCE/Makefile.in" \
   && grep -q 'SBOS excludes the unsupported POSIX spawn source modules' "$SOURCE/Makefile.in" \
   && grep -q 'does not use pselect' "$SOURCE/lib/gnulib.mk" \
   && grep -q 'does not use pselect' "$SOURCE/Makefile.in"; then
    echo "SBOS Gnulib stdio patch already applied"
    exit 0
fi

cd "$SOURCE"
patch --batch --forward -p1 < "$PATCH_FILE"
patch --batch --forward -p1 < "$MODULE_PATCH_FILE"
patch --batch --forward -p1 < "$ROOT/tools/patches/coreutils-9.12-spawn.patch"
patch --batch --forward -p1 < "$ROOT/tools/patches/coreutils-9.12-posix-optional.patch"
