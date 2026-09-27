# SBOS handoff

## Current design

SBOS boots the Rust x86_64 kernel through the EFI loader. The kernel loads the
ELF passed as `shell.elf` and starts it in Ring 3. `shell.elf` is now upstream
GNU Bash 5.3 (official patchlevel 20); the Rust `sbsh` package has been removed.
The initial process runs as root in `/Users/Root`, and Bash can launch external
ELFs from `/Applications`.

The normal build is `tools/build.ps1`. It builds the C runtime, POSIX probe,
Bash 5.3, the selected Coreutils 9.12 profile, applets, kernel and EFI loader.
The official source archive is local
input at `_qemu/downloads/bash-5.3.tar.gz`; extracted upstream sources stay in
ignored `_qemu/src/`. Port patches and configure cache live under
`ports/bash-5.3/`.

## Current implemented state

* The native shell startup path now enters Bash directly with argv/envp.
* GNU Coreutils 9.12 applets are exposed from one static ELF under their
  `/Applications` names: `[`, `basename`, `cat`, `date`, `dirname`, `env`,
  `false`, `head`, `ls`, `printenv`, `printf`, `pwd`, `tail`, `tee`, `test`,
  `tr`, `true`, `wc`, and `yes`. C `clear`, `id`, `mv`, fault and probe
  programs remain separate.
* `rename()` is implemented through POSIX syscall 63, VFS and both filesystem
  backends. SBFS journals source/destination directory entry changes and
  metadata together. Replacing an existing file is supported.
* The ELF process-entry stack now points at `argc`; the C runtime aligns its
  stack before calling `main`.
* Bash 5.3.20 builds and links with the freestanding SBOS C runtime. Bash is
  built without Readline, job control, NLS and multibyte locale support.
* The optional PCI/e1000 + smoltcp stack boots on QEMU and obtains DHCPv4.
  Ring 3 IPv4 UDP supports `socket/bind/connect/sendto/recvfrom` and fd
  `read/write`. libc `getaddrinfo` uses DHCP DNS for IPv4 A records; QEMU
  resolved `example.com` to `172.66.147.243`. TCP and reverse DNS remain
  unimplemented.
* The `driver-rtc` feature reads CMOS RTC at boot. A kernel time service
  combines its Unix epoch with PIT ticks and exposes realtime/monotonic
  `clock_gettime`; libc `time` and `gettimeofday` use that syscall.
* Both `tools/build.ps1` and `date` runtime smoke passed. See
  `build/time-smoke-transcript.log`: QEMU reports a valid RTC epoch, prints
  `date -u` calendar/epoch output, then passes `posix-probe` and exits Bash.
  The general shell smoke transcript is `build/session-transcript.log`.

## Next work

1. Add TCP stream socket semantics and port more GNU utilities. The resolver
   currently handles IPv4 A records only; complete legacy host APIs and
   reverse lookup after TCP is available.
2. Continue Coreutils/GNU ports (grep, sed, findutils, tar, gzip) by enabling
   commands only after their real syscall contracts and QEMU behavior work.
3. Extend locale beyond C/POSIX and add a timezone database/policy. The current
   CMOS source assumes UTC; SBFS metadata timestamps still use TSC counts.
4. Continue libc gaps: `mkdir`, `chmod`, and `fchmod` currently return
   `ENOSYS`.
5. Add Readline only after its required termcap/terminal capabilities are
   implemented and verified. Job control and signal delivery remain incomplete.
6. Investigate nvi only after the Bash/TTY baseline is stable; it still needs
   curses/terminfo and Berkeley DB support.

Do not claim complete POSIX compliance. Keep unsupported calls explicit and
keep all external upstream source archives out of Git.
