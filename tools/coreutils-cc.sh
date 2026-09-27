#!/bin/sh
# Cross compiler driver for GNU Coreutils targeting the SBOS freestanding ABI.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CLANG=${SBOS_CLANG:-clang}
# Gnulib's generated <sys/socket.h> shadows the still-minimal target header;
# Coreutils only needs the standard IPv4 family number for inet_pton here.
SBOS_TARGET_DEFINES='-D__SBOS__=1 -DAF_INET=2 -DHAVE___FPURGE=1'
for argument in "$@"; do
    case "$argument" in
        -c|-E|-S|--version|-version|-dumpmachine|-print-*)
            exec "$CLANG" --target=x86_64-unknown-none-elf -std=gnu11 \
                -ffreestanding -fno-builtin -fno-stack-protector -fno-pic \
                -mno-red-zone -D_POSIX_VERSION=200809L $SBOS_TARGET_DEFINES "$@" \
                -isystem "$ROOT/libc/include" \
                -isystem "$ROOT/user/runtime/include"
            ;;
    esac
done

exec "$CLANG" --target=x86_64-unknown-none-elf -std=gnu11 \
    -ffreestanding -fno-builtin -fno-stack-protector -fno-pic \
    -mno-red-zone -D_POSIX_VERSION=200809L $SBOS_TARGET_DEFINES \
    -fuse-ld=lld -nostdlib -static -no-pie \
    -Wl,--build-id=none -Wl,-z,max-page-size=0x1000 \
    "-Wl,-T,$ROOT/user/runtime/linker.ld" \
    "$@" -isystem "$ROOT/libc/include" \
    -isystem "$ROOT/user/runtime/include" \
    "$ROOT/build/libc/crt0.o" "$ROOT/build/libc/libsbos.a"
