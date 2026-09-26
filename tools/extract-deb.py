#!/usr/bin/env python3
"""Extract only the x86_64 OVMF flash images from a Debian .deb archive."""
import io
import sys
import tarfile
from pathlib import Path

AR_MAGIC = b"!<arch>\n"
WANTED = {
    "usr/share/OVMF/OVMF_CODE_4M.fd": "OVMF_CODE_4M.fd",
    "usr/share/OVMF/OVMF_VARS_4M.fd": "OVMF_VARS_4M.fd",
}


def members(data: bytes):
    if not data.startswith(AR_MAGIC):
        raise ValueError("not an ar/deb archive")
    cursor = len(AR_MAGIC)
    while cursor + 60 <= len(data):
        header = data[cursor:cursor + 60]
        if header[58:60] != b"`\n":
            raise ValueError("invalid ar member header")
        name = header[:16].decode("ascii").strip().rstrip("/")
        size = int(header[48:58].decode("ascii").strip())
        start = cursor + 60
        yield name, data[start:start + size]
        cursor = start + size + (size & 1)


def main(package: Path, destination: Path):
    package_data = package.read_bytes()
    data_tar = next(content for name, content in members(package_data) if name.startswith("data.tar"))
    destination.mkdir(parents=True, exist_ok=True)
    found = set()
    with tarfile.open(fileobj=io.BytesIO(data_tar), mode="r:xz") as archive:
        for member in archive.getmembers():
            name = member.name.removeprefix("./")
            if name not in WANTED:
                continue
            source = archive.extractfile(member)
            if source is None:
                raise ValueError(f"missing data for {member.name}")
            output = destination / WANTED[name]
            output.write_bytes(source.read())
            found.add(name)
    if found != set(WANTED):
        raise ValueError(f"OVMF images missing from package: {set(WANTED) - found}")
    print(f"Extracted OVMF code and variable template to {destination}")


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]))
