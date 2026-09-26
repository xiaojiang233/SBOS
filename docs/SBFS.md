# SBFS v1

SBFS is the native persistent filesystem. It stores user and application files and directories; devices, processes, services, and configuration remain in their own managers and APIs.

## Device path

```text
DeviceManager → BlockDevice → VolumeManager → SBFS → VFS
```

The first implementation uses a 512-byte `BlockDevice`. QEMU attaches a 64 MiB raw IDE image at `build/sbfs.img`. SBFS consumes the whole block device; it does not parse a partition table. A disk whose first block is all zeroes is formatted. An unknown non-empty disk is rejected rather than overwritten.

## On-disk layout

| Region | Contents |
| --- | --- |
| Block 0 | Superblock: version, geometry, Volume UUID, root NodeId, and region offsets |
| Following blocks | Allocation bitmap, one bit per device block |
| Fixed node table | 256 records of 256 bytes each |
| Journal | Header, target-block list, and up to 16 redo sectors |
| Remaining blocks | File and directory data |

NodeIds are 128 bits. The high 64 bits use the volume UUID prefix; the low 64 bits contain a node-table slot and its generation. Deleted slots can be reused without making stale VFS handles refer to the replacement node.

Node records store type, size, owner and group IDs, a four-entry ACL, timestamps, flags, an extended-metadata pointer, and up to eight extents. Directories use 64-byte records with a NodeId and a UTF-8 name of at most 44 bytes. A zero NodeId marks a deleted entry; directory slots are not compacted in v1.

## Journal behavior

Metadata transactions write the redo payload and target list, flush them, write and flush a committed journal header, apply the target sectors, flush again, and clear the header. Mount replays a committed transaction before reading the allocation bitmap. File payloads are written and flushed before metadata points at newly allocated blocks; they are not included in the journal, so v1 does not promise atomic multi-block file data writes.

## ACL and timestamps

An ACL contains up to four subject entries: Owner, Group, Everyone, or a specific User ID. Each entry grants the SBOS read, write, and execute capabilities. This is not a POSIX mode field. The current VFS and POSIX open paths check these grants.

Timestamps are stored as CPU TSC counts until SBOS has a wall-clock source. Creation and modification times are updated; access time is present in the format but is not updated yet.

## Current limits

- One raw volume, 512-byte blocks, and 256 node slots.
- Eight extents per node and 44-byte UTF-8 names.
- Metadata journal supports at most 16 changed sectors per transaction.
- No symlinks, sparse files, checksums, snapshots, compression, encryption, CoW, partition discovery, or recovery from damaged metadata.
- ATA PIO driver is a polling legacy IDE implementation for QEMU bring-up; it is not an AHCI or general hardware storage driver.

The format reserves unused node-record bytes for future metadata without assigning those bytes semantics in v1.
