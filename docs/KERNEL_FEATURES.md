# Kernel feature profiles

SBOS remains one monolithic kernel image, while optional hardware drivers and
filesystem backends are selected at compile time. Modules communicate through
kernel-owned interfaces such as `BlockDevice`, `NetworkDevice`, `Filesystem`,
and the device/volume managers.

## Feature map

| Feature | Enables | Dependencies |
| --- | --- | --- |
| `driver-pci` | Legacy PCI configuration mechanism 1 scanner | none |
| `driver-e1000` | QEMU 82540EM PCI Ethernet driver | `driver-pci`, `network-stack` |
| `network-stack` | smoltcp no_std protocol engine and SBOS E1000 adapter | optional smoltcp crate |
| `driver-ata` | ATA PIO `BlockDevice` driver | none |
| `driver-ps2` | PS/2 keyboard input | none |
| `driver-rtc` | CMOS RTC source for realtime clock | x86 CMOS ports |
| `driver-framebuffer` | UEFI GOP framebuffer renderer | boot-provided GOP framebuffer |
| `fs-sbfs` | Persistent SBFS implementation and VFS backend | a registered block device at runtime |
| `fs-tmpfs` | tmpfs implementation and VFS backend | none |
| `qemu` | Reference boot image | all listed drivers and both FS backends |

At least one of `fs-sbfs` or `fs-tmpfs` must be selected. Enabling both allows
SBFS on disk with tmpfs fallback when no disk is attached. Selecting only
`fs-tmpfs` removes the SBFS implementation from the image.

## Building profiles

The normal full build uses the `qemu` profile:

```powershell
./tools/build.ps1
```

The build script disables Cargo defaults and passes the chosen complete feature
set. For example:

```powershell
./tools/build.ps1 -KernelFeatures fs-tmpfs
./tools/build.ps1 -KernelFeatures fs-sbfs,fs-tmpfs,driver-ata,driver-ps2
```

The first command produces a serial-console/tmpfs system without ATA, PCI,
e1000, PS/2, RTC or framebuffer driver code. The second retains both filesystems,
disk support and PS/2 while omitting network code. Select `driver-e1000` to
include the QEMU Ethernet driver and its protocol stack.

## Current limits

Process management, syscall dispatch, serial console, ELF loading and the
interactive Bash boot path are still required parts of the kernel image. They
do not yet have independent Cargo feature switches. Driver and filesystem
selection is being added first because these are the interfaces currently
under active bring-up.
