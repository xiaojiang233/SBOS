# Upstream userspace on SBOS

## GNU Bash 5.3

SBOS builds the official GNU Bash 5.3 release with upstream patchlevel 20 and
starts it directly as the first Ring 3 process. The ELF Loader passes
`argc`/`argv`/`envp`; Bash runs as root in `/Users/Root`, with `/Applications`
on `PATH`. The old Rust `sbsh` crate is removed. `mv`, `ls`, `cat`, `clear`,
and `id` are separate C programs installed in `/Applications`.

The source archive is local build input at `_qemu/downloads/bash-5.3.tar.gz`.
`tools/prepare-bash53.sh` unpacks it into ignored `_qemu/src/bash-5.3`, applies
the 15 official GNU patches and SBOS port patches, and saves the upstream
`COPYING` text in `ports/bash-5.3/`. `tools/build-bash53.sh` cross-builds the
static x86_64 ELF with the SBOS C runtime. `tools/build.ps1` runs this as part
of the normal build and copies the result to `build/esp/shell.elf`.

The port disables GNU Readline, job control, NLS and multibyte locale support.
The shell uses the canonical SBOS TTY, so command input and builtins work, but
interactive line editing, history recall and job control are not available.
`HISTFILE` starts empty because the current C runtime has no real-time clock or
permission-changing calls. Network redirection is unavailable and returns
`ENOSYS`.

## POSIX C runtime and filesystem calls

`libc/` builds a freestanding C runtime and `userland/posix-probe.c` checks the
syscall surface from a real Ring 3 process. Implemented paths currently cover
file descriptors, `fork`, `execve`, `waitpid`, pipes, terminal attributes,
`TIOCGWINSZ`, `stat`/`fstat`, and `rename`.

`rename()` routes through syscall 63 into VFS. tmpfs updates its node tree;
SBFS changes both directory entries and node metadata in one metadata-journal
transaction, including replacing an existing destination file. Directory
moves reject cycles and non-empty destination directories. Symlinks, hard
links, open-unlinked-file lifetime, and cross-volume rename are not implemented.

## Verification

Run the full build with `tools/build.ps1`. The headless smoke session boots a
private QEMU disk, enters Bash directly, checks shell builtins and external
utilities, replaces a file with `mv`, and runs the POSIX probe:

```powershell
./tools/qemu-session.ps1 -Session tools/sessions/shell-smoke.txt `
    -Log build/session-bash53-shell-final.log -BootSeconds 14 -RunSeconds 42
```

`qemu-session.ps1` uses its own SBFS image and OVMF variables, and stops only
the QEMU process it starts. See the generated transcript for the exact output
of the current build.

The verified shell smoke transcript starts with `bash-5.3#`, reports Bash
`5.3.20(...)-release`, and shows `pwd` as `/Users/Root`. `ls /Applications`
lists Bash, the selected Coreutils applets, the C utilities and `posix-probe`,
with no Rust shell entry. GNU `ls`, `cat`, and `printf` run as external Ring 3
programs. `mv` replaces an existing target, `cat` reads the replacement
contents, and the POSIX probe reports successful `rename`,
`fork`/pipe/`waitpid`, and `execve` with argv/envp replacement. Bash accepts
another command after the probe and exits cleanly.

A separate time smoke boots the same image and checks GNU `date -u` in both
calendar and epoch forms. The kernel reports a valid CMOS RTC epoch, and the
epoch advances with PIT ticks. The new POSIX syscall backs
`clock_gettime(CLOCK_REALTIME)`, `clock_gettime(CLOCK_MONOTONIC)`, `time()`, and
`gettimeofday()`; realtime is explicitly unavailable (`ENODATA`) if no usable
RTC is present.

The Coreutils 9.12 profile currently includes `[`, `basename`, `cat`, `date`,
`dirname`, `env`, `false`, `head`, `ls`, `printenv`, `printf`, `pwd`, `tail`,
`tee`, `test`, `tr`, `true`, `wc`, and `yes`. Commands that need the missing
incomplete TTY/volume queries, including `stty`, `df`, and `du`, are not
included. The time implementation assumes the CMOS RTC is configured as UTC;
regional timezone data and daylight-saving rules are not available.

## Current C runtime gaps

`mkdir`, `chmod` and `fchmod` explicitly return `ENOSYS` where their kernel
operations are not available. `setlocale` supports the C/POSIX locale only.
These are tracked as unsupported APIs rather than successful placeholder
operations.
