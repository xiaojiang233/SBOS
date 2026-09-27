#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SOURCE="$ROOT/_qemu/src/grep-3.12"
BUILD="$ROOT/_qemu/src/grep-3.12/wsl-build"
JOBS=${JOBS:-2}

sh "$ROOT/tools/prepare-grep.sh"
mkdir -p "$BUILD"
cd "$BUILD"

PROFILE_HASH=$(
    sha256sum "$ROOT/tools/build-grep.sh" "$ROOT/tools/grep-cc.sh" \
        "$ROOT/tools/prepare-grep.sh" "$ROOT/tools/patches/grep-3.12-sbos.patch" \
        "$SOURCE/configure" \
        "$SOURCE/Makefile.in" "$SOURCE/src/Makefile.in" |
        sha256sum | cut -d ' ' -f1
)
PROFILE_FILE="$BUILD/.sbos-configure-profile"

if [ ! -f Makefile ] || [ "$(cat "$PROFILE_FILE" 2>/dev/null || true)" != "$PROFILE_HASH" ]; then
    rm -f config.cache
    gt_cv_locale_fr=none \
    gt_cv_locale_ja=none \
    gt_cv_locale_en_utf8=none \
    gt_cv_locale_zh_CN=none \
    gt_cv_locale_fr_utf8=none \
    gt_cv_locale_fake=no \
    gt_cv_locale_solaris114=no \
    gt_cv_locale_aix72=no \
    gl_cv_func_mbrtowc_null_arg1=yes \
    gl_cv_func_mbrtowc_null_arg2=yes \
    gl_cv_func_mbrtowc_retval=yes \
    gl_cv_func_mbrtowc_nul_retval=yes \
    gl_cv_func_mbrtowc_stores_incomplete=no \
    gl_cv_func_mbrtowc_empty_input=yes \
    gl_cv_func_mbrtowc_C_locale_sans_EILSEQ=yes \
    CC="$ROOT/tools/grep-cc.sh" \
    AR=ar \
    RANLIB=ranlib \
    CFLAGS='-O2 -D_POSIX_VERSION=200809L -DNO_MULTIBYTE_SUPPORT=1' \
    "$SOURCE/configure" \
        --cache-file=config.cache \
        --build=x86_64-pc-linux-gnu \
        --host=x86_64-unknown-none \
        --disable-nls \
        --disable-perl-regexp \
        --with-included-regex
    printf '%s\n' "$PROFILE_HASH" >"$PROFILE_FILE"
fi

make -j"$JOBS" -C lib
if [ -f src/grep ] && [ "$ROOT/build/libc/libsbos.a" -nt src/grep ]; then
    rm -f src/grep
fi
make -j"$JOBS" -C src grep
mkdir -p "$ROOT/build/grep"
cp src/grep "$ROOT/build/grep/grep.elf"
echo "build-grep: installed $ROOT/build/grep/grep.elf"
