#!/bin/sh
# Cross-build upstream GNU Bash for SBOS (x86_64, freestanding, SBFS root).
#
# Usage:
#   sh tools/build-bash.sh          # configure (first run) and build
#   sh tools/build-bash.sh clean    # remove this build tree first
#
# The upstream GNU release tarball is unpacked by hand into
#   _qemu/src/bash-$SBOS_BASH_VERSION
# which is local scratch space and is never committed. Port fixes live in
# ports/bash-$SBOS_BASH_VERSION/ and are applied by this build driver.
#
# This script needs a POSIX shell (MSYS/Git Bash or Linux), GNU make, and
# clang + lld. It does not need WSL.

set -e

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=5.3
PORT=$ROOT/ports/bash-$VERSION
SRC=${SBOS_BASH_SRC:-$ROOT/_qemu/src/bash-$VERSION}
BUILD=${SBOS_BASH_BUILD:-$SRC/sbos-build}
ARCHIVE=$ROOT/build/libc/libsbos-bash.a
JOBS=${SBOS_BASH_JOBS:-4}

die() {
    echo "build-bash: $1" >&2
    exit 1
}

[ -d "$SRC" ] || die "Bash $VERSION source tree not found at $SRC
Unpack the official bash-$VERSION.tar.gz there and prepare ports/bash-$VERSION."
[ -f "$ARCHIVE" ] || die "Bash C runtime archive missing at $ARCHIVE
Run tools/build-libc.ps1 first."

# Prepare the upstream release and apply the port's committed patches.
PREPARE=$ROOT/tools/prepare-bash53.sh
SBOS_BASH_SRC="$SRC" sh "$PREPARE"

# GNU make for Windows runs recipes through a shell and bakes the path it finds
# into $(SHELL), which the generated makefiles then use unquoted. A shell that
# lives under a path with spaces ("C:\Program Files\Git\usr\bin\sh.exe") breaks
# every recipe, so a shell in a space free location is chosen explicitly. On
# Linux the plain /bin/sh is correct.
if [ -n "${MAKE_SHELL:-}" ]; then
    :
elif command -v cygpath >/dev/null 2>&1; then
    MAKE_SHELL=
    for candidate in \
        "$HOME"/.workbuddy/binaries/PortableGit/versions/*/usr/bin/sh.exe \
        /usr/bin/sh.exe
    do
        [ -x "$candidate" ] || continue
        windows=$(cygpath -w "$candidate" 2>/dev/null) || continue
        case "$windows" in
            *" "*) continue ;;
        esac
        MAKE_SHELL=$windows
        break
    done
    if [ -z "$MAKE_SHELL" ]; then
        die "no shell in a space free path was found for make.
Set MAKE_SHELL to the Windows path of an sh.exe without spaces."
    fi
else
    MAKE_SHELL=/bin/sh
fi

if ! command -v make >/dev/null 2>&1; then
    WINGET_LINKS="/c/Users/$USER/AppData/Local/Microsoft/WinGet/Links"
    if [ -d "$WINGET_LINKS" ]; then
        PATH="$WINGET_LINKS:$PATH"
        export PATH
    fi
fi
command -v make >/dev/null 2>&1 || die "GNU make not found.
On Windows: winget install --id ezwinports.make --exact"

# Bash keeps its own view of the environment through lib/sh/getenv.c and its own
# rename/strpbrk/mktime fallbacks, so it must link against the Bash-specific C
# runtime archive rather than the default one.
export SBOS_BASH_PORT=1
CC="$ROOT/tools/bash-cc.sh"
export CC
export CPP="$CC -E"
if command -v cygpath >/dev/null 2>&1; then
    export AR=${AR:-llvm-ar}
    export RANLIB=${RANLIB:-llvm-ranlib}
else
    # WSL uses the native binutils names; the LLVM-suffixed tools may not be
    # installed even though clang/LLD provide the target compiler and linker.
    export AR=${AR:-ar}
    export RANLIB=${RANLIB:-ranlib}
fi
# Host tools (mksyntax, mkbuiltins, bashversion) must run on the build machine,
# so they are compiled with the host compiler instead of the target driver.
# They include the SBOS config.h, so they also need the host compatibility
# declarations. The include path is given in its Windows form: arguments that
# reach a native compiler through make are not always path converted for us.
HOST_COMPAT=$PORT/host-compat.h
if command -v cygpath >/dev/null 2>&1; then
    HOST_COMPAT=$(cygpath -m "$HOST_COMPAT")
fi
if [ -z "${CC_FOR_BUILD:-}" ]; then
    if command -v cygpath >/dev/null 2>&1; then
        # Windows-hosted generators need the MinGW compatibility declarations.
        CC_FOR_BUILD="clang -std=gnu89 -include $HOST_COMPAT"
    else
        # Under WSL/Linux, generators must be native Linux executables. The
        # compatibility header only contributes Bash's target identity macros
        # here; its MinGW declarations are guarded by _WIN32.
        CC_FOR_BUILD="gcc -std=gnu89 -include $HOST_COMPAT"
    fi
fi
export CC_FOR_BUILD

case "$1" in
    clean)
        # Reconfiguring only needs config.h to be gone, and the sources decide
        # what gets recompiled. The whole tree is moved aside rather than
        # deleted so the script never has to remove hundreds of files at once
        # (which some sandboxes refuse); delete the stale trees by hand when
        # they are no longer wanted.
        if [ -d "$BUILD" ]; then
            stale="$BUILD.stale.$$"
            mv "$BUILD" "$stale"
            echo "build-bash: moved the previous build tree to $stale"
        fi
        shift
        ;;
esac

mkdir -p "$BUILD"
cd "$BUILD"

# Autoconf and its generated config.status create helper sed scripts under
# $TMPDIR and pass those paths to sed. When TMPDIR is a Windows path such as
# C:\Users\...\Temp the helpers cannot be opened and every substitution in the
# generated makefiles is silently lost, so a POSIX temporary directory is used.
TMPDIR="$BUILD/tmp"
mkdir -p "$TMPDIR"
export TMPDIR
export TMP="$TMPDIR"
export TEMP="$TMPDIR"

if [ ! -f config.h ]; then
    # Bash's configure runs its probe programs. SBOS cannot execute host-built
    # binaries, so every result a cross build should observe is pinned in the
    # cache script instead of being detected.
    #
    # configure must be reached through a relative path: the generated Makefile
    # bakes $srcdir into its rules, and GNU make for Windows cannot resolve a
    # POSIX absolute path such as /d/Coding/... in a prerequisite.
    [ "$(dirname -- "$BUILD")" = "$SRC" ] || die "the build directory must sit
inside the Bash source tree so configure can be invoked as ../configure
(source: $SRC, build: $BUILD)"
    echo "build-bash: configuring in $BUILD"
    READLINE_OPTION=
    if [ "${SBOS_BASH_DISABLE_READLINE:-0}" = 1 ]; then
        READLINE_OPTION=--disable-readline
        echo "build-bash: disabling GNU Readline by request"
    else
        echo "build-bash: enabling bundled GNU Readline"
    fi
    (
        cd "$BUILD"
        . "$PORT/sbos-configure-cache.sh"
        # configure must not inherit an interactive stdin: a probe that reads
        # from it would block the build forever.
        ../configure \
            --build=x86_64-pc-linux-gnu \
            --host=x86_64-unknown-none \
            --disable-nls \
            --without-bash-malloc \
            $READLINE_OPTION \
            --disable-job-control </dev/null
    )
fi

# make regenerates config.h through config.status when stamp-h is older than
# the configuration inputs, and config.status needs a working TMPDIR. That step
# has already happened: restore the header if it went missing, then keep the
# stamp ahead of everything so make never rewrites generated files during the
# build itself.
if [ ! -f config.h ] && [ -f config.status ]; then
    echo "build-bash: regenerating config.h from config.status"
    ( cd "$BUILD" && TMPDIR="$TMPDIR" sh ./config.status >/dev/null 2>&1 </dev/null ) || true
fi
[ -f config.h ] || die "config.h is missing in $BUILD and config.status could not
recreate it. Run: sh tools/build-bash.sh clean"
# Bash's out-of-tree object rules compile headers that live in the source tree;
# quoted includes prefer the source directory over -I., so mirror the selected
# target configuration there before compiling.
cp config.h "$SRC/config.h"
touch stamp-h

# configure records the build directory as an absolute POSIX path. GNU make for
# Windows cannot resolve "/d/Coding/..." in a prerequisite, so BUILD_DIR in the
# generated sub-Makefiles is rewritten to the mixed "D:/Coding/..." form, which
# both make and the MSYS shell understand.
if command -v cygpath >/dev/null 2>&1; then
    root_win=$(cygpath -m "$ROOT")
    find "$BUILD" -name Makefile -type f -exec sed -i "s|$ROOT|$root_win|g" {} +
    sed -i "s|$ROOT|$root_win|g" "$BUILD/config.status"
fi

# SYSTEM_FLAGS carries the identity macros for the shell sources. Its values are
# single-quoted shell words containing double quotes, and GNU make for Windows
# hands such a recipe to the shell as one lump instead of separate arguments, so
# no object file can be compiled. The definitions are supplied by
# tools/bash-cc.sh instead; see the comment there.
sed -i 's|^SYSTEM_FLAGS = .*|SYSTEM_FLAGS =|' "$BUILD/Makefile"

# config.h is produced by configure from the pinned cache, so make must not try
# to regenerate it through config.status during the build: that step rewrites
# generated files, needs a working TMPDIR, and has already been done.
sed -i 's|CONFIG_FILES= CONFIG_HEADERS=config.h \$(SHELL) ./config.status|@true|' "$BUILD/Makefile"

echo "build-bash: building with $CC"
# SBOS_PORT_DIR lets the Bash makefiles reach the port's own helper scripts
# (currently the pipesize.h generator, which replaces an unrunnable host probe).
PORT_DIR="$PORT"
# version.h is generated by support/mkversion.sh, and shell.h includes it. The
# object rules do not list it as a prerequisite, so a parallel build can start
# compiling sources before it exists. Generate it serially first.
# TMPDIR is passed explicitly: config.status and configure write helper scripts
# there and hand the paths to sed, so it must reach the recipe environment.
make SHELL="$MAKE_SHELL" CC_FOR_BUILD="$CC_FOR_BUILD" SBOS_PORT_DIR="$PORT_DIR" TMPDIR="$TMPDIR" version.h </dev/null
make -j "$JOBS" SHELL="$MAKE_SHELL" CC_FOR_BUILD="$CC_FOR_BUILD" SBOS_PORT_DIR="$PORT_DIR" TMPDIR="$TMPDIR" "$@" </dev/null

# The kernel embeds this file and installs it as /Applications/bash.
if [ -f "$BUILD/bash" ]; then
    mkdir -p "$ROOT/build/bash"
    cp "$BUILD/bash" "$ROOT/build/bash/bash.elf"
    echo "build-bash: installed $ROOT/build/bash/bash.elf"
fi
