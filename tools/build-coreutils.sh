#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SOURCE="$ROOT/_qemu/src/coreutils-9.12/coreutils-9.12"
BUILD="$ROOT/_qemu/src/coreutils-9.12/wsl-build"
JOBS=${JOBS:-2}

if [ ! -x "$SOURCE/configure" ]; then
    echo "Coreutils 9.12 source archive is not extracted: $SOURCE" >&2
    exit 1
fi

sh "$ROOT/tools/prepare-coreutils.sh" "$SOURCE"
mkdir -p "$BUILD"
cd "$BUILD"

# configure rewrites generated makefiles. Running it on every normal build
# makes make rebuild the entire imported Gnulib archive, so fingerprint all
# inputs that define this target profile and configure only when they change.
PROFILE_HASH=$(
    sha256sum "$ROOT/tools/build-coreutils.sh" "$ROOT/tools/prepare-coreutils.sh" \
        "$ROOT/tools/coreutils-cc.sh" "$SOURCE/configure" \
        "$ROOT"/tools/patches/coreutils-9.12-*.patch |
        sha256sum | cut -d ' ' -f1
)
PROFILE_FILE="$BUILD/.sbos-configure-profile"

if [ ! -f Makefile ] || [ "$(cat "$PROFILE_FILE" 2>/dev/null || true)" != "$PROFILE_HASH" ]; then
gt_cv_locale_fr=none \
gt_cv_locale_ja=none \
gt_cv_locale_en_utf8=none \
gt_cv_locale_zh_CN=none \
gt_cv_locale_fr_utf8=none \
gt_cv_locale_fake=no \
gt_cv_locale_solaris114=no \
gt_cv_locale_aix72=no \
gl_cv_socklen_t_equiv=int \
gl_cv_func_strerror_0_works=yes \
gl_cv_func_strtod_works='no (underflow problem)' \
ac_cv_header_sys_select_h=yes \
CC="$ROOT/tools/coreutils-cc.sh" \
CFLAGS='-O2 -D_POSIX_VERSION=200809L -DNO_MULTIBYTE_SUPPORT=1' \
"$SOURCE/configure" \
    --cache-file=config.cache \
    --build=x86_64-pc-linux-gnu \
    --host=x86_64-unknown-none \
    --disable-nls \
    --without-selinux \
    --without-systemd \
    --without-wtmpdb \
    --enable-single-binary=hardlinks \
    --enable-no-install-program=b2sum,base64,base32,basenc,chgrp,chmod,chown,cksum,comm,cp,csplit,dd,dir,dircolors,du,echo,expand,expr,factor,fmt,fold,ginstall,groups,id,join,link,ln,logname,md5sum,mkfifo,mknod,mktemp,mv,nl,nproc,nohup,numfmt,od,paste,pathchk,pr,ptx,readlink,realpath,sha1sum,sha224sum,sha256sum,sha384sum,sha512sum,shred,shuf,sort,split,stat,stty,sum,sync,tac,touch,truncate,tsort,tty,uname,unexpand,uniq,unlink,vdir,whoami
    printf '%s\n' "$PROFILE_HASH" >"$PROFILE_FILE"
fi

make -j"$JOBS" src/coreutils
