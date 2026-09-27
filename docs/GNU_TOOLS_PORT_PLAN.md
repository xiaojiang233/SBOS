# GNU userland port plan

This tracks the GNU userland bring-up on SBOS. GNU Bash 5.3 is the Ring 3
interactive shell, and a selected Coreutils 9.12 single-binary profile now
provides external commands. Later candidates are grep, sed, findutils,
diffutils, tar and gzip. Network clients such as wget wait for the Ring 3
socket and DNS APIs.

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

The initial package profile uses Coreutils' single-binary build mode. SBOS does
not currently implement symlinks or hardlinks, so the build will install the
same ELF image at each selected `/Applications/<command>` name. Bash supplies
the command path as `argv[0]`, allowing Coreutils to select its applet without
changing SBOS's ELF loader or VFS object model.

Current upstream references: [Coreutils 9.12 source](https://ftp.gnu.org/gnu/coreutils/coreutils-9.12.tar.xz), [GNU Gnulib manual](https://www.gnu.org/software/gnulib/manual/gnulib.html), [grep 3.12 release](https://lists.gnu.org/archive/html/info-gnu/2025-04/msg00008.html), [sed 4.10 release](https://lists.gnu.org/archive/html/info-gnu/2026-04/msg00009.html), and [findutils 4.10.0 release](https://lists.gnu.org/archive/html/bug-findutils/2024-06/msg00017.html).

## Current command profile

The built Coreutils ELF is installed under these `/Applications` names:
`[`, `basename`, `cat`, `date`, `dirname`, `env`, `false`, `head`, `ls`, `printenv`,
`printf`, `pwd`, `tail`, `tee`, `test`, `tr`, `true`, `wc`, and `yes`. Bash
chooses the applet from `argv[0]`. The separate C `clear`, `id`, `mv`, fault,
channel and POSIX probe programs remain available.

Do not enable commands whose current contract is still missing at the syscall
layer, such as full `mkdir`, `rmdir`, file mode changes, timestamps, symlinks,
hardlinks, `stat -f`, or host accounting data. Add those native/POSIX operations
as individually scoped compatibility work before widening the package list.

## Configure findings

The Coreutils 9.12 configure script accepts `x86_64-unknown-none` as a cross
host. Its selected profile compiles and links as a static ELF64 image using
the SBOS libc. The current libc intentionally lacks many optional POSIX interfaces, including
`*at` filesystem calls, `fchdir`, `fchown`, `futimens`, `ftruncate`, `getgroups`,
`fseeko`/`ftello`, locale conversion, TCP sockets and several legacy resolver
APIs. Configure reports these
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

Coreutils is a deliberately selected command profile, not a complete GNU
Coreutils port. `date -u` was QEMU-tested against the RTC/PIT time service.
`stty`, `df`, and `du` remain excluded because their required
TTY/volume interfaces are incomplete. Grep, sed, findutils, tar and gzip have
not yet been ported. Ring 3 IPv4 UDP and libc `getaddrinfo` A-record lookup
work; TCP streams and reverse/legacy resolver APIs remain missing, so typical
network clients cannot run yet. Locale
support remains C/POSIX only, Bash
Readline, job control, NLS, and multibyte locale support remain disabled, and
unsupported libc calls continue to report their actual errors.
