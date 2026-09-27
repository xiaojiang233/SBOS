# GNU userland port plan

This tracks the GNU userland bring-up on SBOS. GNU Bash 5.3 is the Ring 3
interactive shell, a selected Coreutils 9.12 single-binary profile provides
external commands, and GNU grep 3.12 plus GNU sed 4.10 are separate Ring 3
programs. Later candidates are findutils, diffutils, tar and gzip. Network
clients such as wget wait for the Ring 3 socket and DNS APIs.

## Source versions and build model

The audit downloaded the official Coreutils 9.12 source archive and detached
signature into the ignored `_qemu/downloads/gnu-tools/` directory. Its SHA-256
matches the checksum published with the maintainer's 2026-09-14 stable release
announcement. The extracted source and generated configure tree are under the
ignored `_qemu/src/coreutils-9.12/` directory.

Coreutils, grep and sed use Gnulib for shared portability code. Gnulib is
imported into each package's source tree; it is not a normal external library
that SBOS can link as one installed `.a` archive. That is useful for filling in
missing host functions, but it also means we need to control each package's
configured feature set and syscall expectations.

The Coreutils profile uses single-binary mode; Bash supplies each selected
command path as `argv[0]`. GNU grep and sed are built separately and installed
at `/Applications/grep` and `/Applications/sed`. All are static ELF64 programs
that use the SBOS C runtime and VFS without depending on symlinks or hardlinks.

Current upstream references: [Coreutils 9.12 source](https://ftp.gnu.org/gnu/coreutils/coreutils-9.12.tar.xz), [GNU Gnulib manual](https://www.gnu.org/software/gnulib/manual/gnulib.html), [grep 3.12 release](https://lists.gnu.org/archive/html/info-gnu/2025-04/msg00008.html), [sed 4.10 source](https://ftp.gnu.org/gnu/sed/sed-4.10.tar.xz), and [findutils 4.10.0 release](https://lists.gnu.org/archive/html/bug-findutils/2024-06/msg00017.html).

## Current command profile

The built Coreutils ELF is installed under these `/Applications` names:
`[`, `basename`, `cat`, `cut`, `date`, `dirname`, `env`, `false`, `head`,
`ls`, `mkdir`, `printenv`, `printf`, `pwd`, `rm`, `rmdir`, `seq`, `sleep`,
`tail`, `tee`, `test`, `tr`, `true`, `wc`, and `yes`. Bash chooses the applet
from `argv[0]`. The separate C `clear`, `id`, `mv`, desktop, mouse, network,
TCP, and POSIX probe programs remain available alongside GNU grep 3.12 and
GNU sed 4.10.

`mkdir`, `rm`, and `rmdir` were added after their VFS syscall paths were
available. Keep excluding commands whose contracts remain incomplete, such as
file mode changes, timestamps, symlinks, hardlinks, `stat -f`, and host
accounting data.

## Configure findings

The Coreutils 9.12 configure script accepts `x86_64-unknown-none` as a cross
host. Its selected profile compiles and links as a static ELF64 image using
the SBOS libc. The current libc intentionally lacks many optional POSIX interfaces, including
`*at` filesystem calls, `fchdir`, `fchown`, `futimens`, `ftruncate`, `getgroups`,
`fseeko`/`ftello`, locale conversion, broader TCP options and several legacy
resolver APIs. Configure reports these
as absent so Gnulib can select substitutes. The shell smoke session exercised
the installed `ls`, `cat`, and `printf` applets under Ring 3. The new kernel
time service exposes CMOS RTC UTC realtime plus PIT-backed monotonic time. The
QEMU time smoke verified GNU `date -u` calendar formatting and Unix epoch
output; the epoch progresses with PIT ticks.

The Coreutils profile disables NLS, SELinux, systemd, and wtmpdb. ACL and
multithreaded-sort support are disabled by the target's missing host APIs. The
selected package profile builds successfully. Porting more utilities will
continue to surface missing libc and VFS contracts.

## Current status and limits

`tools/build.ps1` completes through EFI/ELF artifact verification. A QEMU
smoke run booted through UEFI, mounted persistent SBFS, brought up DHCP, entered
Ring 3 Bash, listed `/Applications`, read `/System/Readme.txt`, and exercised
Coreutils `printf`, `ls`, and `cat`. It also verified the existing-file `mv`
flow and the POSIX probe's fork/pipe/waitpid/execve checks; Bash remained
interactive afterward and exited cleanly.
The separate GNU grep 3.12 smoke checks `--version`, basic and extended
regular expressions, `-n`, `-i`, `-v`, `-c`, and `-q` exit status against an
SBFS file.
GNU sed 4.10 has a QEMU smoke for `--version`, global basic substitutions,
line-number selection with `-n`, and anchored expression selection.

Coreutils is a deliberately selected command profile, not a complete GNU
Coreutils port. `date -u` was QEMU-tested against the RTC/PIT time service.
`stty`, `df`, and `du` remain excluded because their required
TTY/volume interfaces are incomplete. GNU grep and sed are single-byte C-locale
builds; findutils, diffutils, tar and gzip have not yet been ported. Ring 3 IPv4 UDP
and libc `getaddrinfo` A-record lookup
work; TCP stream listen/accept/read/write works in QEMU, while active connect
and broader stream semantics still need coverage. Reverse/legacy resolver APIs
remain missing, so typical network clients are not yet compatible. Locale
support remains C/POSIX only. Bash now uses its bundled GNU Readline and
termcap libraries, while job control, NLS, and multibyte locale support remain
disabled. Unsupported libc calls continue to report their actual errors.
