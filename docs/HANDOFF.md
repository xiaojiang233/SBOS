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
* GNU grep 3.12 is built as a separate static ELF using imported Gnulib regex
  support and is installed as `/Applications/grep`. Its upstream archive's
  detached signature was verified; QEMU smoke covers BRE/ERE and selection
  flags against an SBFS file.
* GNU sed 4.10 is built as a separate static ELF and installed at
  `/Applications/sed`. QEMU smoke verifies GNU version output, substitutions,
  line-address printing and anchored address selection.
* `rename()` is implemented through POSIX syscall 63, VFS and both filesystem
  backends. SBFS journals source/destination directory entry changes and
  metadata together. Replacing an existing file is supported.
* The ELF process-entry stack now points at `argc`; the C runtime aligns its
  stack before calling `main`.
* Bash 5.3.20 builds with bundled GNU Readline and termcap against the
  freestanding SBOS C runtime. SBOS supplies an inline ANSI `sbos` termcap
  entry; QMP key injection verified left-arrow insertion into a command.
  Job control, NLS and multibyte locale support remain disabled.
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
* A Ring 3 X11 11.0 subset now listens on TCP/6000 in explicit `-noauth`
  development mode. `tools/x11-probe.py` completed setup, CreateWindow,
  MapWindow, CreateGC, PolyFillRectangle, and GetGeometry through a QEMU
  localhost-only host-forward. QMP injection verified keyboard KeyPress/
  KeyRelease plus pointer MotionNotify/ButtonPress delivery. See
  `build/x11-pointer-smoke.log`.

## Next work

1. Expand the X11 subset with client events and pixmaps, then implement a
   compositor/window manager. Add authentication before offering remote X11
   access; the current server explicitly disables it for the private QEMU test.
2. Add broader TCP semantics and GNU utilities. The resolver currently handles
   IPv4 A records only; complete legacy host APIs and reverse lookup.
3. Continue GNU ports (sed, findutils, diffutils, tar, gzip) by enabling
   commands only after their real syscall contracts and QEMU behavior work.
4. Extend locale beyond C/POSIX and add a timezone database/policy. The current
   CMOS source assumes UTC; SBFS metadata timestamps still use TSC counts.
5. Continue libc gaps: `chmod` and `fchmod` currently return
   `ENOSYS`.
6. Expand terminal behavior and the C runtime around Readline; interactive
   job control and signal delivery remain incomplete. The current Readline
   profile is single-byte and has no persistent history file.
7. Investigate nvi only after the Bash/TTY baseline is stable; it still needs
   curses/terminfo and Berkeley DB support.

Do not claim complete POSIX compliance. Keep unsupported calls explicit and
keep all external upstream source archives out of Git.
