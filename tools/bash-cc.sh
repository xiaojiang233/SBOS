#!/bin/sh
# Compiler driver used for the GNU Bash 5.3 port.
#
# It applies the SBOS freestanding flags, points the compiler at the SBOS
# headers, and links the result against the SBOS C runtime. Bash keeps its own
# environment functions and its own rename/strpbrk/mktime fallbacks, so
# SBOS_BASH_PORT=1 selects the Bash-specific archive built by
# tools/build-libc.ps1 instead of the default one.

SBOS_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SBOS_LIB=libsbos.a
if [ "${SBOS_BASH_PORT:-0}" = 1 ]; then
    SBOS_LIB=libsbos-bash.a
fi

# Bash's makefile builds its identity macros out of quoted values:
#
#   SYSTEM_FLAGS = -DPROGRAM='"$(Program)"' -DCONF_MACHTYPE='"$(MACHTYPE)"' ...
#
# GNU make for Windows passes such a recipe to the shell as a single argument,
# so the compiler never sees the individual -c/-o/-I words. tools/build-bash.sh
# therefore empties SYSTEM_FLAGS in the generated makefile, and the definitions
# are supplied here instead. The single quotes protect the double quotes from
# being stripped by this shell; they become part of the -D value, which is what
# the Bash sources expect.
SBOS_IDENTITY='-DPROGRAM="bash" -DPACKAGE="bash" -DLOCALEDIR="/System/Locale" -D_POSIX_VERSION=200809L -DCONF_HOSTTYPE="x86_64" -DCONF_OSTYPE="none" -DCONF_MACHTYPE="x86_64-unknown-none" -DCONF_VENDOR="unknown"'

for argument in "$@"; do
    case "$argument" in
        -c|-E|-S|--version|-version|-dumpmachine|-print-*)
            # shellcheck disable=SC2086
            exec clang --target=x86_64-unknown-none-elf -std=gnu89 \
                -ffreestanding -fno-builtin -fno-stack-protector -fno-pic \
                -mno-red-zone -DCLK_TCK=100 -DNO_MULTIBYTE_SUPPORT=1 -DNEED_EXTERN_PC=1 -I"$SBOS_ROOT/libc/include" \
                -I"$SBOS_ROOT/user/runtime/include" -include time.h -include wchar.h -include strings.h \
                $SBOS_IDENTITY "$@"
            ;;
    esac
done

exec clang --target=x86_64-unknown-none-elf -std=gnu89 \
    -ffreestanding -fno-builtin -fno-stack-protector -fno-pic \
    -mno-red-zone -DCLK_TCK=100 -DNO_MULTIBYTE_SUPPORT=1 -DNEED_EXTERN_PC=1 -I"$SBOS_ROOT/libc/include" \
    -I"$SBOS_ROOT/user/runtime/include" -include time.h -include wchar.h -include strings.h \
    -fuse-ld=lld -nostdlib -static \
    -Wl,--build-id=none -Wl,-z,max-page-size=0x1000 \
    -Wl,-e,_start "-Wl,-T,$SBOS_ROOT/user/runtime/linker.ld" \
    "$@" "$SBOS_ROOT/build/libc/crt0.o" "$SBOS_ROOT/build/libc/$SBOS_LIB"
