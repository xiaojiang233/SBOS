#!/usr/bin/env python3
"""Check the load contracts shared by the UEFI loader and kernel."""
import struct
import sys
from pathlib import Path


def elf(path: Path):
    data = path.read_bytes()
    if len(data) < 64 or data[:4] != b"\x7fELF" or data[4:6] != b"\x02\x01":
        raise ValueError(f"{path.name}: expected little-endian ELF64")
    kind, machine = struct.unpack_from("<HH", data, 16)
    entry = struct.unpack_from("<Q", data, 24)[0]
    phoff = struct.unpack_from("<Q", data, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", data, 54)
    if machine != 62 or phentsize != 56 or phoff + phentsize * phnum > len(data):
        raise ValueError(f"{path.name}: invalid x86_64 program-header table")
    segments = []
    for i in range(phnum):
        p_type, flags, offset, vaddr, paddr, filesz, memsz, align = struct.unpack_from("<IIQQQQQQ", data, phoff + i * phentsize)
        if p_type == 1 and memsz:
            if filesz > memsz or offset + filesz > len(data):
                raise ValueError(f"{path.name}: invalid PT_LOAD extent")
            segments.append((flags, vaddr, paddr, filesz, memsz, align))
    if kind != 2:
        raise ValueError(f"{path.name}: expected ET_EXEC (type 2), got {kind}")
    if not any(flags & 1 and start <= entry < start + memsz for flags, start, _, _, memsz, _ in segments):
        raise ValueError(f"{path.name}: entry is not in an executable PT_LOAD")
    return entry, segments


def main(esp: Path):
    kernel_entry, kernel_segments = elf(esp / "kernel.elf")
    shell_entry, shell_segments = elf(esp / "shell.elf")
    if kernel_entry >= 0x40000000 or any(paddr + memsz > 0x40000000 for _, _, paddr, _, memsz, _ in kernel_segments):
        raise ValueError("kernel physical image must fit below 1 GiB")
    if shell_entry < 0x10000 or shell_entry >= 0x0000800000000000:
        raise ValueError("shell entry is outside the lower canonical user range")
    if any((flags & 3) == 3 for flags, *_ in shell_segments):
        raise ValueError("shell contains a writable and executable segment")

    pe = (esp / "EFI/BOOT/BOOTX64.EFI").read_bytes()
    if len(pe) < 256 or pe[:2] != b"MZ":
        raise ValueError("BOOTX64.EFI is not PE/COFF")
    peoff = struct.unpack_from("<I", pe, 0x3C)[0]
    if pe[peoff:peoff + 4] != b"PE\0\0":
        raise ValueError("BOOTX64.EFI has an invalid PE signature")
    machine = struct.unpack_from("<H", pe, peoff + 4)[0]
    optional_size = struct.unpack_from("<H", pe, peoff + 20)[0]
    optional = peoff + 24
    subsystem = struct.unpack_from("<H", pe, optional + 68)[0]
    entry_rva = struct.unpack_from("<I", pe, optional + 16)[0]
    if machine != 0x8664 or subsystem != 10 or entry_rva == 0 or optional_size < 70:
        raise ValueError("BOOTX64.EFI must be an x86_64 EFI application")
    print(f"EFI application verified; kernel entry {kernel_entry:#x}, shell entry {shell_entry:#x}")


if __name__ == "__main__":
    try:
        main(Path(sys.argv[1]))
    except Exception as exc:
        print(f"artifact verification failed: {exc}", file=sys.stderr)
        raise SystemExit(1)
