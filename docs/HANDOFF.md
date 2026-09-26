# SBOS handoff

## Current design

SBOS boots the Rust x86_64 kernel through the EFI loader. The kernel loads the
ELF passed as `shell.elf` and starts it in Ring 3. `shell.elf` is now upstream
GNU Bash 5.3 (official patchlevel 20); the Rust `sbsh` package has been removed.
The initial process runs as root in `/Users/Root`, and Bash can launch external
ELFs from `/Applications`.

The normal build is `tools/build.ps1`. It builds the C runtime, POSIX probe,
Bash 5.3, applets, kernel and EFI loader. The official source archive is local
input at `_qemu/downloads/bash-5.3.tar.gz`; extracted upstream sources stay in
ignored `_qemu/src/`. Port patches and configure cache live under
`ports/bash-5.3/`.

## Implemented this turn

* The native shell startup path now enters Bash directly with argv/envp.
* `ls`, `cat`, `clear`, `id` and `mv` are standalone C programs in
  `/Applications`.
* `rename()` is implemented through POSIX syscall 63, VFS and both filesystem
  backends. SBFS journals source/destination directory entry changes and
  metadata together. Replacing an existing file is supported.
* The ELF process-entry stack now points at `argc`; the C runtime aligns its
  stack before calling `main`.
* Bash 5.3.20 builds and links with the freestanding SBOS C runtime. Bash is
  built without Readline, job control, NLS and multibyte locale support.

## Next work

1. The QEMU smoke run in `tools/sessions/shell-smoke.txt` passed with the
   current source. Its transcript is `build/session-bash53-shell-final.log`;
   `/Applications/sbsh` was removed from the persistent SBFS image.
2. Continue libc gaps from concrete program needs. `gettimeofday`, `mkdir`,
   `chmod` and `fchmod` currently return `ENOSYS`; locale is limited to C/POSIX.
3. Add Readline only after its required termcap/terminal capabilities are
   implemented and verified. Job control and signal delivery remain incomplete.
4. Investigate nvi only after the Bash/TTY baseline is stable; it still needs
   curses/terminfo and Berkeley DB support.

Do not claim complete POSIX compliance. Keep unsupported calls explicit and
keep all external upstream source archives out of Git.
