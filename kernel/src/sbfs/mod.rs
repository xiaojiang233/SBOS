//! SBFS v1: a small, sector-based filesystem with stable 128-bit node IDs.
//!
//! The on-disk format uses a fixed node table, a volume allocation bitmap,
//! inline extents, fixed-size UTF-8 directory records, and a redo journal for
//! metadata-sector updates. File payload writes are ordered before metadata
//! commits and are intentionally not data-journaled.

use crate::fs::vnode::{Acl, AclEntry, AclPrincipal, NodeId, VNodeKind, VNodeMetadata};
use crate::device::{BlockDevice, BlockError};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

const BLOCK_SIZE: usize = 512;
const SB_MAGIC: &[u8; 8] = b"SBFSV1\0\0";
const JOURNAL_MAGIC: &[u8; 8] = b"SBJNL1\0\0";
const VERSION: u32 = 1;
const NODE_COUNT: u32 = 256;
const NODE_SIZE: usize = 256;
const NODE_TABLE_BLOCKS: u64 = NODE_COUNT as u64 * NODE_SIZE as u64 / BLOCK_SIZE as u64;
const MAX_EXTENTS: usize = 8;
const EXTENT_SIZE: usize = 12;
const DIR_ENTRY_SIZE: usize = 64;
pub const MAX_NAME_BYTES: usize = 44;
const MAX_JOURNAL_WRITES: usize = 16;
const JOURNAL_BLOCKS: u32 = 2 + MAX_JOURNAL_WRITES as u32;
const NODE_FLAG_EXECUTABLE: u32 = 1;

type Sector = [u8; BLOCK_SIZE];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Extent {
    start: u64,
    blocks: u32,
}
impl Extent {
    const EMPTY: Self = Self { start: 0, blocks: 0 };
}

#[derive(Clone, Copy)]
struct Layout {
    total_blocks: u64,
    uuid: [u8; 16],
    root: NodeId,
    bitmap_start: u64,
    bitmap_blocks: u32,
    nodes_start: u64,
    journal_start: u64,
    journal_blocks: u32,
    data_start: u64,
}

#[derive(Clone)]
struct NodeRecord {
    valid: bool,
    id: NodeId,
    parent: NodeId,
    kind: VNodeKind,
    size: u64,
    uid: u32,
    gid: u32,
    acl: Acl,
    created: u64,
    modified: u64,
    accessed: u64,
    flags: u32,
    extended_metadata: u64,
    extents: [Extent; MAX_EXTENTS],
    extent_count: usize,
}
impl NodeRecord {
    fn empty() -> Self {
        Self {
            valid: false,
            id: NodeId([0; 16]),
            parent: NodeId([0; 16]),
            kind: VNodeKind::File,
            size: 0,
            uid: 0,
            gid: 0,
            acl: Acl::EMPTY,
            created: 0,
            modified: 0,
            accessed: 0,
            flags: 0,
            extended_metadata: 0,
            extents: [Extent::EMPTY; MAX_EXTENTS],
            extent_count: 0,
        }
    }

    fn encode(&self, output: &mut [u8]) {
        output.fill(0);
        output[0] = u8::from(self.valid);
        output[1] = match self.kind {
            VNodeKind::File => 1,
            VNodeKind::Directory => 2,
        };
        output[2] = self.extent_count as u8;
        put_u32(output, 4, self.flags);
        put_u64(output, 8, self.size);
        output[16..32].copy_from_slice(&self.id.0);
        output[32..48].copy_from_slice(&self.parent.0);
        put_u32(output, 48, self.uid);
        put_u32(output, 52, self.gid);
        output[56] = self.acl.count;
        for (index, entry) in self.acl.entries.iter().take(self.acl.count as usize).enumerate() {
            let offset = 60 + index * 8;
            let (kind, subject) = match entry.principal {
                AclPrincipal::Owner => (1, 0),
                AclPrincipal::Group => (2, 0),
                AclPrincipal::Everyone => (3, 0),
                AclPrincipal::User(uid) => (4, uid),
            };
            output[offset] = kind;
            put_u32(output, offset + 1, subject);
            put_u16(output, offset + 5, entry.rights);
        }
        put_u64(output, 92, self.created);
        put_u64(output, 100, self.modified);
        put_u64(output, 108, self.accessed);
        for (index, extent) in self.extents.iter().take(self.extent_count).enumerate() {
            let offset = 116 + index * EXTENT_SIZE;
            put_u64(output, offset, extent.start);
            put_u32(output, offset + 8, extent.blocks);
        }
        put_u64(output, 216, self.extended_metadata);
    }

    fn decode(input: &[u8]) -> Result<Self, &'static str> {
        if input.len() < NODE_SIZE {
            return Err("short SBFS node record");
        }
        let valid = input[0] == 1;
        let kind = match input[1] {
            1 => VNodeKind::File,
            2 => VNodeKind::Directory,
            _ => VNodeKind::File,
        };
        let extent_count = input[2] as usize;
        if extent_count > MAX_EXTENTS {
            return Err("SBFS node has too many extents");
        }
        let acl_count = input[56] as usize;
        if acl_count > 4 {
            return Err("SBFS node has too many ACL entries");
        }
        let mut acl = Acl::EMPTY;
        acl.count = acl_count as u8;
        for index in 0..acl_count {
            let offset = 60 + index * 8;
            acl.entries[index] = AclEntry {
                principal: match input[offset] {
                    1 => AclPrincipal::Owner,
                    2 => AclPrincipal::Group,
                    3 => AclPrincipal::Everyone,
                    4 => AclPrincipal::User(get_u32(input, offset + 1)),
                    _ => return Err("SBFS node contains an invalid ACL principal"),
                },
                rights: get_u16(input, offset + 5),
            };
        }
        let mut id = [0u8; 16];
        id.copy_from_slice(&input[16..32]);
        let mut parent = [0u8; 16];
        parent.copy_from_slice(&input[32..48]);
        let mut record = Self {
            valid,
            id: NodeId(id),
            parent: NodeId(parent),
            kind,
            size: get_u64(input, 8),
            uid: get_u32(input, 48),
            gid: get_u32(input, 52),
            acl,
            created: get_u64(input, 92),
            modified: get_u64(input, 100),
            accessed: get_u64(input, 108),
            flags: get_u32(input, 4),
            extended_metadata: get_u64(input, 216),
            extents: [Extent::EMPTY; MAX_EXTENTS],
            extent_count,
        };
        for index in 0..extent_count {
            let offset = 116 + index * EXTENT_SIZE;
            record.extents[index] = Extent {
                start: get_u64(input, offset),
                blocks: get_u32(input, offset + 8),
            };
        }
        Ok(record)
    }

    fn allocated_blocks(&self) -> u64 {
        self.extents[..self.extent_count]
            .iter()
            .map(|extent| extent.blocks as u64)
            .sum()
    }

    fn handle(&self) -> u64 {
        self.id.handle()
    }
}

pub struct Sbfs {
    device: Arc<dyn BlockDevice>,
    layout: Layout,
    bitmap: Vec<u8>,
}

impl Sbfs {
    pub fn mount_or_format(device: Arc<dyn BlockDevice>) -> Result<Self, &'static str> {
        if device.block_size() as usize != BLOCK_SIZE {
            return Err("SBFS requires a 512-byte block device");
        }
        if device.block_count() < 1024 {
            return Err("SBFS volume is too small");
        }
        let mut superblock = read_sector(&device, 0)?;
        if superblock.iter().all(|byte| *byte == 0) {
            Self::format(&device)?;
            superblock = read_sector(&device, 0)?;
        }
        if &superblock[0..8] != SB_MAGIC {
            return Err("disk is not a blank SBFS volume");
        }
        let layout = decode_layout(&superblock, device.block_count())?;
        replay_journal(&device, &layout)?;

        let bitmap_len = layout.bitmap_blocks as usize * BLOCK_SIZE;
        let mut bitmap = vec![0u8; bitmap_len];
        device
            .read_blocks(layout.bitmap_start, &mut bitmap)
            .map_err(|_| "failed to read SBFS allocation bitmap")?;
        let fs = Self { device, layout, bitmap };
        let root = fs.read_slot(0)?;
        if !root.valid || root.kind != VNodeKind::Directory || root.id != fs.layout.root {
            return Err("SBFS root node is invalid");
        }
        Ok(fs)
    }

    pub fn sync(&self) -> Result<(), &'static str> {
        self.device.flush().map_err(|_| "failed to flush SBFS volume")
    }

    fn format(device: &Arc<dyn BlockDevice>) -> Result<(), &'static str> {
        let total_blocks = device.block_count();
        let bitmap_blocks = total_blocks
            .checked_add(4095)
            .ok_or("SBFS volume is too large")?
            / 4096;
        let bitmap_blocks = u32::try_from(bitmap_blocks).map_err(|_| "SBFS bitmap is too large")?;
        let nodes_start = 1 + bitmap_blocks as u64;
        let journal_start = nodes_start + NODE_TABLE_BLOCKS;
        let data_start = journal_start + JOURNAL_BLOCKS as u64;
        if data_start >= total_blocks {
            return Err("SBFS metadata does not fit on volume");
        }

        let uuid = make_uuid(total_blocks);
        let root = make_node_id(&uuid, 0, 1);
        let layout = Layout {
            total_blocks,
            uuid,
            root,
            bitmap_start: 1,
            bitmap_blocks,
            nodes_start,
            journal_start,
            journal_blocks: JOURNAL_BLOCKS,
            data_start,
        };

        let mut bitmap = vec![0u8; bitmap_blocks as usize * BLOCK_SIZE];
        for block in 0..data_start {
            set_bit(&mut bitmap, block, true);
        }
        device
            .write_blocks(layout.bitmap_start, &bitmap)
            .map_err(|_| "failed to initialize SBFS bitmap")?;

        let table_bytes = NODE_COUNT as usize * NODE_SIZE;
        let mut node_table = vec![0u8; table_bytes];
        let now = crate::arch::x86_64::port::rdtsc();
        let mut root_node = NodeRecord::empty();
        root_node.valid = true;
        root_node.id = root;
        root_node.parent = root;
        root_node.kind = VNodeKind::Directory;
        root_node.uid = 0;
        root_node.gid = 0;
        root_node.acl = Acl::OWNER_DEFAULT;
        root_node.created = now;
        root_node.modified = now;
        root_node.encode(&mut node_table[..NODE_SIZE]);
        device
            .write_blocks(layout.nodes_start, &node_table)
            .map_err(|_| "failed to initialize SBFS node table")?;

        let empty_journal = vec![0u8; layout.journal_blocks as usize * BLOCK_SIZE];
        device
            .write_blocks(layout.journal_start, &empty_journal)
            .map_err(|_| "failed to initialize SBFS journal")?;
        let mut superblock = [0u8; BLOCK_SIZE];
        encode_layout(&layout, &mut superblock);
        device
            .write_blocks(0, &superblock)
            .map_err(|_| "failed to write SBFS superblock")?;
        device.flush().map_err(|_| "failed to flush SBFS format")?;
        Ok(())
    }

    pub fn volume_uuid(&self) -> [u8; 16] {
        self.layout.uuid
    }

    pub fn root_id(&self) -> u64 {
        self.layout.root.handle()
    }

    pub fn initialize_system_tree(
        &mut self,
        shell_image: &[u8],
        applications: &[(&str, &[u8])],
    ) -> Result<(), &'static str> {
        let root = self.root_id();
        let applications_directory = self.ensure_directory(root, "Applications", 0, Acl::OWNER_DEFAULT)?;
        if let Some(obsolete_shell) = self.lookup_child(applications_directory, "sbsh")? {
            self.remove(obsolete_shell)?;
        }
        let system = self.ensure_directory(root, "System", 0, Acl::OWNER_DEFAULT)?;
        let users = self.ensure_directory(root, "Users", 0, Acl::OWNER_DEFAULT)?;
        self.ensure_directory(root, "Shared", 0, Acl::SHARED_DEFAULT)?;
        self.ensure_directory(root, "Volumes", 0, Acl::OWNER_DEFAULT)?;
        let guest = self.ensure_directory(users, "Guest", 1000, Acl::SHARED_DEFAULT)?;
        self.ensure_file(
            system,
            "Readme.txt",
            b"SBOS uses SBFS for persistent files. Devices, processes, services, and configuration are managed by separate system APIs.\n",
            false,
            0,
            Acl::OWNER_DEFAULT,
        )?;
        self.ensure_file(
            guest,
            "Welcome.txt",
            b"Welcome to SBOS. Try `help` for shell commands.\n",
            false,
            1000,
            Acl::SHARED_DEFAULT,
        )?;
        self.ensure_file(
            applications_directory,
            "bash",
            shell_image,
            true,
            0,
            Acl::OWNER_DEFAULT,
        )?;
        for (name, image) in applications {
            if image.starts_with(b"\x7fELF") {
                self.ensure_file(
                    applications_directory,
                    name,
                    image,
                    true,
                    0,
                    Acl::OWNER_DEFAULT,
                )?;
            }
        }
        Ok(())
    }

    fn ensure_directory(
        &mut self,
        parent: u64,
        name: &str,
        owner: u32,
        acl: Acl,
    ) -> Result<u64, &'static str> {
        if let Some(handle) = self.lookup_child(parent, name)? {
            if self.read_node(handle)?.kind != VNodeKind::Directory {
                return Err("SBFS system path is not a directory");
            }
            return Ok(handle);
        }
        self.create_node(parent, name, VNodeKind::Directory, false, owner, owner, acl)
    }

    fn ensure_file(
        &mut self,
        parent: u64,
        name: &str,
        data: &[u8],
        executable: bool,
        owner: u32,
        acl: Acl,
    ) -> Result<u64, &'static str> {
        let handle = if let Some(handle) = self.lookup_child(parent, name)? {
            if self.read_node(handle)?.kind != VNodeKind::File {
                return Err("SBFS system path is not a file");
            }
            handle
        } else {
            self.create_node(parent, name, VNodeKind::File, executable, owner, owner, acl)?
        };
        if !self.contents_equal(handle, data)? {
            self.replace_contents(handle, data)?;
        }
        Ok(handle)
    }

    fn contents_equal(&self, handle: u64, expected: &[u8]) -> Result<bool, &'static str> {
        let node = self.read_node(handle)?;
        if node.size != expected.len() as u64 {
            return Ok(false);
        }
        let mut buffer = [0u8; 512];
        let mut offset = 0;
        while offset < expected.len() {
            let amount = buffer.len().min(expected.len() - offset);
            let read = self.read_at(handle, offset, &mut buffer[..amount])?;
            if read != amount || buffer[..amount] != expected[offset..offset + amount] {
                return Ok(false);
            }
            offset += amount;
        }
        Ok(true)
    }

    fn replace_contents(&mut self, handle: u64, data: &[u8]) -> Result<(), &'static str> {
        let mut current = self.read_node(handle)?;
        if current.kind != VNodeKind::File {
            return Err("not a file");
        }
        let max_size = usize::try_from(self.layout.total_blocks.saturating_sub(self.layout.data_start))
            .unwrap_or(usize::MAX)
            .saturating_mul(BLOCK_SIZE);
        if data.len() > max_size {
            return Err("replacement exceeds available SBFS space");
        }
        let old_extents = current.extents;
        let old_extent_count = current.extent_count;
        let mut bitmap = self.bitmap.clone();
        current.extents = [Extent::EMPTY; MAX_EXTENTS];
        current.extent_count = 0;
        current.size = 0;
        let needed = data.len().checked_add(BLOCK_SIZE - 1).ok_or("file is too large")? / BLOCK_SIZE;
        ensure_logical_blocks(&self.layout, &mut bitmap, &mut current, needed)?;
        for logical in 0..needed {
            let lba = map_logical_block(&current, logical).ok_or("invalid replacement extent")?;
            let mut sector = [0u8; BLOCK_SIZE];
            let start = logical * BLOCK_SIZE;
            let end = (start + BLOCK_SIZE).min(data.len());
            sector[..end - start].copy_from_slice(&data[start..end]);
            self.device
                .write_blocks(lba, &sector)
                .map_err(|_| "failed to write replacement data")?;
        }
        self.device.flush().map_err(|_| "failed to flush replacement data")?;
        for extent in old_extents.iter().take(old_extent_count) {
            for block in extent.start..extent.start + extent.blocks as u64 {
                set_bit(&mut bitmap, block, false);
            }
        }
        current.size = data.len() as u64;
        current.modified = crate::arch::x86_64::port::rdtsc();
        let mut transaction = Transaction::new(&self.device);
        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, handle as u32, &current)?;
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(())
    }

    pub fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str> {
        if path.is_empty() || path.len() > 512 || cwd.len() > 512 {
            return Err("invalid path");
        }
        let absolute = if path.starts_with('/') {
            String::from(path)
        } else {
            let mut joined = String::from(cwd);
            if !joined.ends_with('/') {
                joined.push('/');
            }
            joined.push_str(path);
            joined
        };
        let mut current = self.root_id();
        for component in absolute.split('/') {
            if component.is_empty() || component == "." {
                continue;
            }
            if component == ".." {
                let node = self.read_node(current)?;
                current = self.handle_for_id(node.parent)?;
                continue;
            }
            current = self.lookup_child(current, component)?.ok_or("path not found")?;
        }
        Ok(current)
    }

    fn lookup_child(&self, parent_handle: u64, name: &str) -> Result<Option<u64>, &'static str> {
        let parent = self.read_node(parent_handle)?;
        if parent.kind != VNodeKind::Directory {
            return Err("not a directory");
        }
        let entries = parent.size as usize / DIR_ENTRY_SIZE;
        let mut sector = [0u8; BLOCK_SIZE];
        let mut cached_lba = u64::MAX;
        for index in 0..entries {
            let offset = index * DIR_ENTRY_SIZE;
            let logical = offset / BLOCK_SIZE;
            let lba = map_logical_block(&parent, logical).ok_or("invalid SBFS directory extent")?;
            if lba != cached_lba {
                self.device
                    .read_blocks(lba, &mut sector)
                    .map_err(|_| "failed to read SBFS directory")?;
                cached_lba = lba;
            }
            let entry = &sector[offset % BLOCK_SIZE..offset % BLOCK_SIZE + DIR_ENTRY_SIZE];
            if entry[0..16].iter().all(|byte| *byte == 0) {
                continue;
            }
            let name_len = entry[17] as usize;
            if name_len == name.len() && name_len <= MAX_NAME_BYTES && &entry[20..20 + name_len] == name.as_bytes() {
                let mut id = [0u8; 16];
                id.copy_from_slice(&entry[0..16]);
                let id = NodeId(id);
                if let Ok(handle) = self.handle_for_id(id) {
                    return Ok(Some(handle));
                }
            }
        }
        Ok(None)
    }

    fn handle_for_id(&self, id: NodeId) -> Result<u64, &'static str> {
        let handle = id.handle();
        if self.read_node(handle).map(|node| node.id) == Ok(id) {
            Ok(handle)
        } else {
            Err("SBFS node no longer exists")
        }
    }

    fn directory_entry_offset(&self, directory: &NodeRecord, id: NodeId) -> Result<usize, &'static str> {
        if directory.kind != VNodeKind::Directory {
            return Err("not a directory");
        }
        for index in 0..directory.size as usize / DIR_ENTRY_SIZE {
            let entry = self.read_directory_entry(directory, index)?;
            if entry[0..16] == id.0 {
                return Ok(index * DIR_ENTRY_SIZE);
            }
        }
        Err("SBFS directory entry is missing")
    }

    fn read_node(&self, handle: u64) -> Result<NodeRecord, &'static str> {
        let slot = handle as u32;
        if slot >= NODE_COUNT {
            return Err("SBFS node handle is out of range");
        }
        let record = self.read_slot(slot)?;
        if !record.valid || record.handle() != handle {
            return Err("file not found");
        }
        Ok(record)
    }

    fn read_slot(&self, slot: u32) -> Result<NodeRecord, &'static str> {
        if slot >= NODE_COUNT {
            return Err("SBFS node slot is out of range");
        }
        let lba = self.layout.nodes_start + slot as u64 / 2;
        let sector = read_sector(&self.device, lba)?;
        let offset = (slot as usize % 2) * NODE_SIZE;
        NodeRecord::decode(&sector[offset..offset + NODE_SIZE])
    }

    pub fn metadata(&self, handle: u64) -> Option<VNodeMetadata> {
        let node = self.read_node(handle).ok()?;
        let size = usize::try_from(node.size).ok()?;
        Some(VNodeMetadata {
            id: handle,
            node_id: node.id,
            kind: node.kind,
            size,
            executable: node.flags & NODE_FLAG_EXECUTABLE != 0,
            uid: node.uid,
            gid: node.gid,
            acl: node.acl,
            created: node.created,
            modified: node.modified,
            accessed: node.accessed,
            flags: node.flags,
        })
    }

    pub fn name(&self, handle: u64) -> Option<String> {
        let node = self.read_node(handle).ok()?;
        if node.id == self.layout.root {
            return Some(String::from("/"));
        }
        let parent = self.handle_for_id(node.parent).ok()?;
        let parent = self.read_node(parent).ok()?;
        let entries = parent.size as usize / DIR_ENTRY_SIZE;
        let mut sector = [0u8; BLOCK_SIZE];
        let mut cached_lba = u64::MAX;
        for index in 0..entries {
            let offset = index * DIR_ENTRY_SIZE;
            let logical = offset / BLOCK_SIZE;
            let lba = map_logical_block(&parent, logical)?;
            if cached_lba != lba {
                self.device.read_blocks(lba, &mut sector).ok()?;
                cached_lba = lba;
            }
            let entry = &sector[offset % BLOCK_SIZE..offset % BLOCK_SIZE + DIR_ENTRY_SIZE];
            if entry[0..16] == node.id.0 {
                let len = entry[17] as usize;
                return core::str::from_utf8(entry.get(20..20 + len)?).ok().map(String::from);
            }
        }
        None
    }

    pub fn can_access(&self, handle: u64, uid: u32, gid: u32, mask: u16) -> bool {
        let Ok(node) = self.read_node(handle) else { return false };
        node.acl.allows(node.uid, node.gid, uid, gid, mask)
    }

    pub fn set_owner_acl(&mut self, handle: u64, uid: u32, gid: u32, acl: Acl) -> Result<(), &'static str> {
        if acl.count > 4 {
            return Err("SBFS ACL exceeds four entries");
        }
        let mut node = self.read_node(handle)?;
        if node.uid == uid && node.gid == gid && node.acl == acl {
            return Ok(());
        }
        node.uid = uid;
        node.gid = gid;
        node.acl = acl;
        node.modified = crate::arch::x86_64::port::rdtsc();
        let mut transaction = Transaction::new(&self.device);
        self.stage_node(&mut transaction, handle as u32, &node)?;
        transaction.commit(&self.layout)
    }

    pub fn create_file(
        &mut self,
        parent: u64,
        name: &str,
        executable: bool,
    ) -> Result<u64, &'static str> {
        self.create_node(parent, name, VNodeKind::File, executable, 1000, 1000, Acl::OWNER_DEFAULT)
    }

    pub fn create_file_with_acl(
        &mut self,
        parent: u64,
        name: &str,
        uid: u32,
        gid: u32,
        acl: Acl,
    ) -> Result<u64, &'static str> {
        self.create_node(parent, name, VNodeKind::File, false, uid, gid, acl)
    }

    pub fn create_directory(&mut self, parent: u64, name: &str) -> Result<u64, &'static str> {
        self.create_node(parent, name, VNodeKind::Directory, false, 1000, 1000, Acl::OWNER_DEFAULT)
    }

    fn create_node(
        &mut self,
        parent_handle: u64,
        name: &str,
        kind: VNodeKind,
        executable: bool,
        uid: u32,
        gid: u32,
        acl: Acl,
    ) -> Result<u64, &'static str> {
        validate_name(name)?;
        if self.lookup_child(parent_handle, name)?.is_some() {
            return Err("name already exists");
        }
        let mut parent = self.read_node(parent_handle)?;
        if parent.kind != VNodeKind::Directory {
            return Err("parent is not a directory");
        }
        let (slot, old_generation) = self.find_free_slot()?;
        let generation = old_generation.checked_add(1).ok_or("SBFS node generation exhausted")?.max(1);
        let id = make_node_id(&self.layout.uuid, slot, generation);
        let now = crate::arch::x86_64::port::rdtsc();
        let mut node = NodeRecord::empty();
        node.valid = true;
        node.id = id;
        node.parent = parent.id;
        node.kind = kind;
        node.uid = uid;
        node.gid = gid;
        node.acl = acl;
        node.flags = if executable { NODE_FLAG_EXECUTABLE } else { 0 };
        node.created = now;
        node.modified = now;

        let offset = usize::try_from(parent.size).map_err(|_| "directory is too large")?;
        if offset % DIR_ENTRY_SIZE != 0 {
            return Err("SBFS directory size is invalid");
        }
        let logical = offset / BLOCK_SIZE;
        let mut bitmap = self.bitmap.clone();
        let old_blocks = parent.allocated_blocks();
        ensure_logical_blocks(&self.layout, &mut bitmap, &mut parent, logical + 1)?;
        let data_lba = map_logical_block(&parent, logical).ok_or("failed to allocate directory block")?;
        let mut transaction = Transaction::new(&self.device);
        let mut sector = if logical as u64 >= old_blocks {
            [0u8; BLOCK_SIZE]
        } else {
            transaction.read(data_lba)?
        };
        let mut entry = [0u8; DIR_ENTRY_SIZE];
        entry[0..16].copy_from_slice(&id.0);
        entry[16] = match kind { VNodeKind::File => 1, VNodeKind::Directory => 2 };
        entry[17] = name.len() as u8;
        put_u16(&mut entry, 18, if executable { 1 } else { 0 });
        entry[20..20 + name.len()].copy_from_slice(name.as_bytes());
        let within = offset % BLOCK_SIZE;
        sector[within..within + DIR_ENTRY_SIZE].copy_from_slice(&entry);
        transaction.stage(data_lba, sector);

        parent.size = parent.size.checked_add(DIR_ENTRY_SIZE as u64).ok_or("directory is too large")?;
        parent.modified = now;
        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, parent_handle as u32, &parent)?;
        self.stage_node(&mut transaction, slot, &node)?;
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(id.handle())
    }

    fn find_free_slot(&self) -> Result<(u32, u32), &'static str> {
        for slot in 0..NODE_COUNT {
            let node = self.read_slot(slot)?;
            if !node.valid {
                let generation = (node.handle() >> 32) as u32;
                return Ok((slot, generation));
            }
        }
        Err("SBFS node table is full")
    }

    fn stage_node(
        &self,
        transaction: &mut Transaction<'_>,
        slot: u32,
        node: &NodeRecord,
    ) -> Result<(), &'static str> {
        let lba = self.layout.nodes_start + slot as u64 / 2;
        let mut sector = transaction.read(lba)?;
        let offset = (slot as usize % 2) * NODE_SIZE;
        node.encode(&mut sector[offset..offset + NODE_SIZE]);
        transaction.stage(lba, sector);
        Ok(())
    }

    fn stage_bitmap_changes(
        &self,
        transaction: &mut Transaction<'_>,
        bitmap: &[u8],
    ) -> Result<(), &'static str> {
        for index in 0..self.layout.bitmap_blocks as usize {
            let start = index * BLOCK_SIZE;
            let end = start + BLOCK_SIZE;
            if self.bitmap[start..end] != bitmap[start..end] {
                let mut sector = [0u8; BLOCK_SIZE];
                sector.copy_from_slice(&bitmap[start..end]);
                transaction.stage(self.layout.bitmap_start + index as u64, sector);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, handle: u64) -> Result<(), &'static str> {
        let mut node = self.read_node(handle)?;
        if node.id == self.layout.root {
            return Err("cannot remove SBFS root");
        }
        if node.kind == VNodeKind::Directory && node.size != 0 {
            for index in 0..node.size as usize / DIR_ENTRY_SIZE {
                let entry = self.read_directory_entry(&node, index)?;
                if entry[0..16].iter().any(|byte| *byte != 0) {
                    return Err("directory is not empty");
                }
            }
        }
        let parent_handle = self.handle_for_id(node.parent)?;
        let mut parent = self.read_node(parent_handle)?;
        let entries = parent.size as usize / DIR_ENTRY_SIZE;
        let mut transaction = Transaction::new(&self.device);
        let mut found = false;
        for index in 0..entries {
            let offset = index * DIR_ENTRY_SIZE;
            let logical = offset / BLOCK_SIZE;
            let lba = map_logical_block(&parent, logical).ok_or("invalid SBFS parent directory")?;
            let mut sector = transaction.read(lba)?;
            let within = offset % BLOCK_SIZE;
            if sector[within..within + 16] == node.id.0 {
                sector[within..within + DIR_ENTRY_SIZE].fill(0);
                transaction.stage(lba, sector);
                found = true;
                break;
            }
        }
        if !found {
            return Err("SBFS parent directory entry is missing");
        }
        let mut bitmap = self.bitmap.clone();
        for extent in node.extents.iter().take(node.extent_count) {
            for block in extent.start..extent.start + extent.blocks as u64 {
                set_bit(&mut bitmap, block, false);
            }
        }
        node.valid = false;
        node.size = 0;
        node.extents = [Extent::EMPTY; MAX_EXTENTS];
        node.extent_count = 0;
        parent.modified = crate::arch::x86_64::port::rdtsc();
        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, handle as u32, &node)?;
        self.stage_node(&mut transaction, parent_handle as u32, &parent)?;
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(())
    }

    pub fn rename_node(&mut self, handle: u64, new_parent_handle: u64, new_name: &str) -> Result<(), &'static str> {
        validate_name(new_name)?;
        let mut node = self.read_node(handle)?;
        if node.id == self.layout.root {
            return Err("cannot rename root");
        }
        let old_parent_handle = self.handle_for_id(node.parent)?;
        let mut old_parent = self.read_node(old_parent_handle)?;
        let mut new_parent = if old_parent_handle == new_parent_handle {
            old_parent.clone()
        } else {
            self.read_node(new_parent_handle)?
        };
        if new_parent.kind != VNodeKind::Directory {
            return Err("not a directory");
        }

        if node.kind == VNodeKind::Directory {
            let mut ancestor = new_parent.id;
            for _ in 0..NODE_COUNT {
                if ancestor == node.id {
                    return Err("directory move would create a cycle");
                }
                if ancestor == self.layout.root {
                    break;
                }
                ancestor = self.read_node(self.handle_for_id(ancestor)?)?.parent;
            }
        }

        let destination_handle = self.lookup_child(new_parent_handle, new_name)?;
        if destination_handle == Some(handle) {
            return Ok(());
        }
        let mut destination = if let Some(destination_handle) = destination_handle {
            let target = self.read_node(destination_handle)?;
            if target.kind != node.kind {
                return Err("rename type mismatch");
            }
            if target.kind == VNodeKind::Directory {
                for index in 0..target.size as usize / DIR_ENTRY_SIZE {
                    let entry = self.read_directory_entry(&target, index)?;
                    if entry[0..16].iter().any(|byte| *byte != 0) {
                        return Err("directory is not empty");
                    }
                }
            }
            Some((destination_handle, target))
        } else {
            None
        };

        let source_offset = self.directory_entry_offset(&old_parent, node.id)?;
        let destination_offset = match destination.as_ref() {
            Some((_, target)) => Some(self.directory_entry_offset(&new_parent, target.id)?),
            None => None,
        };
        let mut bitmap = self.bitmap.clone();
        let mut transaction = Transaction::new(&self.device);

        if let Some((_, target)) = destination.as_mut() {
            for extent in target.extents.iter().take(target.extent_count) {
                for block in extent.start..extent.start + extent.blocks as u64 {
                    set_bit(&mut bitmap, block, false);
                }
            }
            target.valid = false;
            target.size = 0;
            target.extents = [Extent::EMPTY; MAX_EXTENTS];
            target.extent_count = 0;
        }

        let same_parent = old_parent_handle == new_parent_handle;
        if !same_parent || destination_offset.is_some() {
            let old_lba = map_logical_block(&old_parent, source_offset / BLOCK_SIZE)
                .ok_or("invalid SBFS source directory entry")?;
            let mut old_sector = transaction.read(old_lba)?;
            let old_within = source_offset % BLOCK_SIZE;
            old_sector[old_within..old_within + DIR_ENTRY_SIZE].fill(0);
            transaction.stage(old_lba, old_sector);
        }

        let entry_offset = if let Some(offset) = destination_offset {
            offset
        } else if same_parent {
            source_offset
        } else {
            let offset = usize::try_from(new_parent.size).map_err(|_| "directory is too large")?;
            if offset % DIR_ENTRY_SIZE != 0 {
                return Err("SBFS directory size is invalid");
            }
            let old_blocks = new_parent.allocated_blocks();
            let logical = offset / BLOCK_SIZE;
            ensure_logical_blocks(&self.layout, &mut bitmap, &mut new_parent, logical + 1)?;
            new_parent.size = new_parent.size.checked_add(DIR_ENTRY_SIZE as u64)
                .ok_or("directory is too large")?;
            let lba = map_logical_block(&new_parent, logical)
                .ok_or("failed to allocate destination directory block")?;
            let mut sector = if logical as u64 >= old_blocks {
                [0u8; BLOCK_SIZE]
            } else {
                transaction.read(lba)?
            };
            let within = offset % BLOCK_SIZE;
            sector[within..within + DIR_ENTRY_SIZE].copy_from_slice(&encode_directory_entry(&node, new_name));
            transaction.stage(lba, sector);
            offset
        };

        if destination_offset.is_some() || same_parent {
            let lba = map_logical_block(&new_parent, entry_offset / BLOCK_SIZE)
                .ok_or("invalid SBFS destination directory entry")?;
            let mut sector = transaction.read(lba)?;
            let within = entry_offset % BLOCK_SIZE;
            sector[within..within + DIR_ENTRY_SIZE].copy_from_slice(&encode_directory_entry(&node, new_name));
            transaction.stage(lba, sector);
        }

        let now = crate::arch::x86_64::port::rdtsc();
        node.parent = new_parent.id;
        node.modified = now;
        old_parent.modified = now;
        if !same_parent {
            new_parent.modified = now;
        }

        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, handle as u32, &node)?;
        self.stage_node(&mut transaction, old_parent_handle as u32, &old_parent)?;
        if !same_parent {
            self.stage_node(&mut transaction, new_parent_handle as u32, &new_parent)?;
        }
        if let Some((target_handle, target)) = destination {
            self.stage_node(&mut transaction, target_handle as u32, &target)?;
        }
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(())
    }

    pub fn truncate(&mut self, handle: u64) -> Result<(), &'static str> {
        let mut node = self.read_node(handle)?;
        if node.kind != VNodeKind::File { return Err("not a file"); }
        let mut bitmap = self.bitmap.clone();
        for extent in node.extents.iter().take(node.extent_count) {
            for block in extent.start..extent.start + extent.blocks as u64 {
                set_bit(&mut bitmap, block, false);
            }
        }
        node.size = 0;
        node.extents = [Extent::EMPTY; MAX_EXTENTS];
        node.extent_count = 0;
        node.modified = crate::arch::x86_64::port::rdtsc();
        let mut transaction = Transaction::new(&self.device);
        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, handle as u32, &node)?;
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(())
    }

    pub fn read_at(&self, handle: u64, offset: usize, output: &mut [u8]) -> Result<usize, &'static str> {
        let node = self.read_node(handle)?;
        if node.kind != VNodeKind::File {
            return Err("not a file");
        }
        let size = usize::try_from(node.size).map_err(|_| "file is too large")?;
        if offset >= size || output.is_empty() {
            return Ok(0);
        }
        let count = output.len().min(size - offset);
        let mut done = 0;
        while done < count {
            let position = offset + done;
            let logical = position / BLOCK_SIZE;
            let within = position % BLOCK_SIZE;
            let amount = (BLOCK_SIZE - within).min(count - done);
            let lba = map_logical_block(&node, logical).ok_or("invalid SBFS file extent")?;
            let mut sector = [0u8; BLOCK_SIZE];
            self.device
                .read_blocks(lba, &mut sector)
                .map_err(|_| "failed to read SBFS file data")?;
            output[done..done + amount].copy_from_slice(&sector[within..within + amount]);
            done += amount;
        }
        Ok(done)
    }

    pub fn write_at(&mut self, handle: u64, offset: usize, input: &[u8]) -> Result<usize, &'static str> {
        if input.is_empty() {
            return Ok(0);
        }
        let mut node = self.read_node(handle)?;
        if node.kind != VNodeKind::File {
            return Err("not a file");
        }
        let end = offset.checked_add(input.len()).ok_or("file is too large")?;
        let max_size = usize::try_from(self.layout.total_blocks.saturating_sub(self.layout.data_start))
            .unwrap_or(usize::MAX)
            .saturating_mul(BLOCK_SIZE);
        if end > max_size {
            return Err("file exceeds available SBFS space");
        }
        let needed_blocks = end.checked_add(BLOCK_SIZE - 1).ok_or("file is too large")? / BLOCK_SIZE;
        let old_blocks = node.allocated_blocks();
        let mut bitmap = self.bitmap.clone();
        ensure_logical_blocks(&self.layout, &mut bitmap, &mut node, needed_blocks)?;

        // Newly allocated blocks are zeroed before they can be referenced by metadata.
        for logical in old_blocks as usize..needed_blocks {
            let lba = map_logical_block(&node, logical).ok_or("invalid allocated SBFS extent")?;
            self.device
                .write_blocks(lba, &[0u8; BLOCK_SIZE])
                .map_err(|_| "failed to initialize SBFS file block")?;
        }

        let mut done = 0;
        while done < input.len() {
            let position = offset + done;
            let logical = position / BLOCK_SIZE;
            let within = position % BLOCK_SIZE;
            let amount = (BLOCK_SIZE - within).min(input.len() - done);
            let lba = map_logical_block(&node, logical).ok_or("invalid SBFS file extent")?;
            let mut sector = [0u8; BLOCK_SIZE];
            if logical < old_blocks as usize && (within != 0 || amount != BLOCK_SIZE) {
                self.device
                    .read_blocks(lba, &mut sector)
                    .map_err(|_| "failed to read SBFS file block")?;
            }
            sector[within..within + amount].copy_from_slice(&input[done..done + amount]);
            self.device
                .write_blocks(lba, &sector)
                .map_err(|_| "failed to write SBFS file data")?;
            done += amount;
        }
        self.device.flush().map_err(|_| "failed to flush SBFS file data")?;

        node.size = node.size.max(end as u64);
        node.modified = crate::arch::x86_64::port::rdtsc();
        let mut transaction = Transaction::new(&self.device);
        self.stage_bitmap_changes(&mut transaction, &bitmap)?;
        self.stage_node(&mut transaction, handle as u32, &node)?;
        transaction.commit(&self.layout)?;
        self.bitmap = bitmap;
        Ok(input.len())
    }

    fn read_directory_entry(&self, node: &NodeRecord, index: usize) -> Result<[u8; DIR_ENTRY_SIZE], &'static str> {
        let offset = index.checked_mul(DIR_ENTRY_SIZE).ok_or("directory is too large")?;
        if offset + DIR_ENTRY_SIZE > node.size as usize {
            return Err("directory entry is out of range");
        }
        let lba = map_logical_block(node, offset / BLOCK_SIZE).ok_or("invalid SBFS directory extent")?;
        let sector = read_sector(&self.device, lba)?;
        let within = offset % BLOCK_SIZE;
        let mut entry = [0u8; DIR_ENTRY_SIZE];
        entry.copy_from_slice(&sector[within..within + DIR_ENTRY_SIZE]);
        Ok(entry)
    }

    pub fn read_directory(&self, handle: u64, output: &mut [u8]) -> Result<usize, &'static str> {
        let node = self.read_node(handle)?;
        if node.kind != VNodeKind::Directory {
            return Err("not a directory");
        }
        let mut used = 0;
        for index in 0..node.size as usize / DIR_ENTRY_SIZE {
            let entry = self.read_directory_entry(&node, index)?;
            if entry[0..16].iter().all(|byte| *byte == 0) {
                continue;
            }
            let name_len = entry[17] as usize;
            if name_len == 0 || name_len > MAX_NAME_BYTES {
                return Err("invalid SBFS directory name");
            }
            let name = core::str::from_utf8(&entry[20..20 + name_len]).map_err(|_| "invalid SBFS UTF-8 name")?;
            let suffix = if entry[16] == 2 { b"/\n".as_slice() } else { b"\n".as_slice() };
            if used + name.len() + suffix.len() > output.len() {
                break;
            }
            output[used..used + name.len()].copy_from_slice(name.as_bytes());
            used += name.len();
            output[used..used + suffix.len()].copy_from_slice(suffix);
            used += suffix.len();
        }
        Ok(used)
    }

    pub fn read_executable(&self, path: &str, cwd: &str) -> Result<Vec<u8>, &'static str> {
        let id = self.resolve(path, cwd)?;
        let node = self.read_node(id)?;
        if node.kind != VNodeKind::File || node.flags & NODE_FLAG_EXECUTABLE == 0 {
            return Err("not an executable file");
        }
        let size = usize::try_from(node.size).map_err(|_| "executable is too large")?;
        let mut image = vec![0u8; size];
        let count = self.read_at(id, 0, &mut image)?;
        if count != size {
            return Err("short SBFS executable read");
        }
        Ok(image)
    }
}

struct Transaction<'a> {
    device: &'a Arc<dyn BlockDevice>,
    writes: Vec<(u64, Sector)>,
}
impl<'a> Transaction<'a> {
    fn new(device: &'a Arc<dyn BlockDevice>) -> Self {
        Self { device, writes: Vec::new() }
    }

    fn read(&self, lba: u64) -> Result<Sector, &'static str> {
        if let Some((_, sector)) = self.writes.iter().find(|(target, _)| *target == lba) {
            return Ok(*sector);
        }
        read_sector(self.device, lba)
    }

    fn stage(&mut self, lba: u64, sector: Sector) {
        if let Some((_, existing)) = self.writes.iter_mut().find(|(target, _)| *target == lba) {
            *existing = sector;
        } else {
            self.writes.push((lba, sector));
        }
    }

    fn commit(self, layout: &Layout) -> Result<(), &'static str> {
        commit_journal(self.device, layout, &self.writes)
    }
}

fn ensure_logical_blocks(
    layout: &Layout,
    bitmap: &mut [u8],
    node: &mut NodeRecord,
    wanted: usize,
) -> Result<(), &'static str> {
    while node.allocated_blocks() < wanted as u64 {
        let previous = if node.extent_count == 0 {
            None
        } else {
            let extent = node.extents[node.extent_count - 1];
            Some(extent.start + extent.blocks as u64)
        };
        let candidate = previous
            .filter(|block| *block < layout.total_blocks && !get_bit(bitmap, *block))
            .or_else(|| (layout.data_start..layout.total_blocks).find(|block| !get_bit(bitmap, *block)))
            .ok_or("SBFS volume is full")?;
        set_bit(bitmap, candidate, true);
        if node.extent_count > 0 {
            let last = &mut node.extents[node.extent_count - 1];
            if last.start + last.blocks as u64 == candidate {
                last.blocks = last.blocks.checked_add(1).ok_or("SBFS extent is too large")?;
                continue;
            }
        }
        if node.extent_count == MAX_EXTENTS {
            set_bit(bitmap, candidate, false);
            return Err("SBFS file exceeds the eight-extent limit");
        }
        node.extents[node.extent_count] = Extent { start: candidate, blocks: 1 };
        node.extent_count += 1;
    }
    Ok(())
}

fn map_logical_block(node: &NodeRecord, logical: usize) -> Option<u64> {
    let mut remaining = logical as u64;
    for extent in node.extents.iter().take(node.extent_count) {
        if remaining < extent.blocks as u64 {
            return Some(extent.start + remaining);
        }
        remaining -= extent.blocks as u64;
    }
    None
}

fn validate_name(name: &str) -> Result<(), &'static str> {
    if name.len() > MAX_NAME_BYTES {
        return Err("name is too long");
    }
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.as_bytes().contains(&0)
        || name.contains('/')
    {
        return Err("invalid SBFS file name");
    }
    Ok(())
}

fn encode_directory_entry(node: &NodeRecord, name: &str) -> [u8; DIR_ENTRY_SIZE] {
    let mut entry = [0u8; DIR_ENTRY_SIZE];
    entry[0..16].copy_from_slice(&node.id.0);
    entry[16] = match node.kind {
        VNodeKind::File => 1,
        VNodeKind::Directory => 2,
    };
    entry[17] = name.len() as u8;
    put_u16(&mut entry, 18, if node.flags & NODE_FLAG_EXECUTABLE != 0 { 1 } else { 0 });
    entry[20..20 + name.len()].copy_from_slice(name.as_bytes());
    entry
}

fn make_node_id(uuid: &[u8; 16], slot: u32, generation: u32) -> NodeId {
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&uuid[..8]);
    bytes[8..12].copy_from_slice(&slot.to_le_bytes());
    bytes[12..16].copy_from_slice(&generation.to_le_bytes());
    NodeId(bytes)
}

fn make_uuid(total_blocks: u64) -> [u8; 16] {
    let mut state = crate::arch::x86_64::port::rdtsc()
        ^ total_blocks.rotate_left(17)
        ^ crate::memory::vmm::current_root().rotate_left(31);
    let mut uuid = [0u8; 16];
    for byte in &mut uuid {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        *byte = state as u8;
    }
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    uuid
}

fn encode_layout(layout: &Layout, output: &mut Sector) {
    output.fill(0);
    output[0..8].copy_from_slice(SB_MAGIC);
    put_u32(output, 8, VERSION);
    put_u32(output, 12, BLOCK_SIZE as u32);
    put_u64(output, 16, layout.total_blocks);
    output[24..40].copy_from_slice(&layout.uuid);
    output[40..56].copy_from_slice(&layout.root.0);
    put_u64(output, 56, layout.bitmap_start);
    put_u32(output, 64, layout.bitmap_blocks);
    put_u64(output, 72, layout.nodes_start);
    put_u32(output, 80, NODE_COUNT);
    put_u32(output, 84, NODE_SIZE as u32);
    put_u64(output, 88, layout.journal_start);
    put_u32(output, 96, layout.journal_blocks);
    put_u64(output, 104, layout.data_start);
}

fn decode_layout(input: &Sector, device_blocks: u64) -> Result<Layout, &'static str> {
    if get_u32(input, 8) != VERSION || get_u32(input, 12) as usize != BLOCK_SIZE {
        return Err("unsupported SBFS version or block size");
    }
    let total_blocks = get_u64(input, 16);
    let bitmap_blocks = get_u32(input, 64);
    let journal_blocks = get_u32(input, 96);
    let nodes_start = get_u64(input, 72);
    let bitmap_start = get_u64(input, 56);
    let journal_start = get_u64(input, 88);
    let data_start = get_u64(input, 104);
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&input[24..40]);
    let mut root = [0u8; 16];
    root.copy_from_slice(&input[40..56]);
    if total_blocks > device_blocks
        || total_blocks < 1024
        || bitmap_start != 1
        || bitmap_blocks as u64 != total_blocks.saturating_add(4095) / 4096
        || get_u32(input, 80) != NODE_COUNT
        || get_u32(input, 84) as usize != NODE_SIZE
        || journal_blocks != JOURNAL_BLOCKS
        || nodes_start != bitmap_start + bitmap_blocks as u64
        || journal_start != nodes_start + NODE_TABLE_BLOCKS
        || data_start != journal_start + journal_blocks as u64
        || data_start >= total_blocks
    {
        return Err("SBFS superblock layout is invalid");
    }
    Ok(Layout {
        total_blocks,
        uuid,
        root: NodeId(root),
        bitmap_start,
        bitmap_blocks,
        nodes_start,
        journal_start,
        journal_blocks,
        data_start,
    })
}

fn commit_journal(
    device: &Arc<dyn BlockDevice>,
    layout: &Layout,
    writes: &[(u64, Sector)],
) -> Result<(), &'static str> {
    if writes.is_empty() {
        return Ok(());
    }
    if writes.len() > MAX_JOURNAL_WRITES {
        return Err("SBFS metadata transaction exceeds journal capacity");
    }
    for (lba, _) in writes {
        if *lba >= layout.total_blocks
            || (*lba >= layout.journal_start && *lba < layout.journal_start + layout.journal_blocks as u64)
        {
            return Err("invalid SBFS journal target");
        }
    }

    let empty = [0u8; BLOCK_SIZE];
    device
        .write_blocks(layout.journal_start, &empty)
        .map_err(|_| "failed to clear SBFS journal header")?;
    device.flush().map_err(|_| "failed to flush SBFS journal header")?;

    let mut targets = [0u8; BLOCK_SIZE];
    let mut checksum = 0xcbf29ce484222325u64;
    for (index, (lba, sector)) in writes.iter().enumerate() {
        put_u64(&mut targets, index * 8, *lba);
        checksum = hash_bytes(checksum, &lba.to_le_bytes());
        checksum = hash_bytes(checksum, sector);
        device
            .write_blocks(layout.journal_start + 2 + index as u64, sector)
            .map_err(|_| "failed to write SBFS journal data")?;
    }
    device
        .write_blocks(layout.journal_start + 1, &targets)
        .map_err(|_| "failed to write SBFS journal targets")?;
    device.flush().map_err(|_| "failed to flush SBFS journal data")?;

    let mut header = [0u8; BLOCK_SIZE];
    header[0..8].copy_from_slice(JOURNAL_MAGIC);
    put_u32(&mut header, 8, VERSION);
    put_u32(&mut header, 12, 1);
    put_u32(&mut header, 16, writes.len() as u32);
    put_u64(&mut header, 24, checksum);
    device
        .write_blocks(layout.journal_start, &header)
        .map_err(|_| "failed to commit SBFS journal")?;
    device.flush().map_err(|_| "failed to flush SBFS journal commit")?;

    for (lba, sector) in writes {
        device
            .write_blocks(*lba, sector)
            .map_err(|_| "failed to apply SBFS metadata transaction")?;
    }
    device.flush().map_err(|_| "failed to flush SBFS metadata")?;
    device
        .write_blocks(layout.journal_start, &empty)
        .map_err(|_| "failed to clear committed SBFS journal")?;
    device.flush().map_err(|_| "failed to flush cleared SBFS journal")?;
    Ok(())
}

fn replay_journal(device: &Arc<dyn BlockDevice>, layout: &Layout) -> Result<(), &'static str> {
    let header = read_sector(device, layout.journal_start)?;
    if &header[0..8] != JOURNAL_MAGIC {
        return Ok(());
    }
    let count = get_u32(&header, 16) as usize;
    if get_u32(&header, 8) != VERSION || get_u32(&header, 12) != 1 || count == 0 || count > MAX_JOURNAL_WRITES {
        return Err("SBFS journal header is invalid");
    }
    let targets = read_sector(device, layout.journal_start + 1)?;
    let mut checksum = 0xcbf29ce484222325u64;
    let mut writes = Vec::with_capacity(count);
    for index in 0..count {
        let lba = get_u64(&targets, index * 8);
        if lba >= layout.total_blocks
            || (lba >= layout.journal_start && lba < layout.journal_start + layout.journal_blocks as u64)
        {
            return Err("SBFS journal target is invalid");
        }
        let sector = read_sector(device, layout.journal_start + 2 + index as u64)?;
        checksum = hash_bytes(checksum, &lba.to_le_bytes());
        checksum = hash_bytes(checksum, &sector);
        writes.push((lba, sector));
    }
    if checksum != get_u64(&header, 24) {
        return Err("SBFS journal checksum mismatch");
    }
    for (lba, sector) in &writes {
        device
            .write_blocks(*lba, sector)
            .map_err(|_| "failed to replay SBFS journal")?;
    }
    device.flush().map_err(|_| "failed to flush SBFS journal replay")?;
    device
        .write_blocks(layout.journal_start, &[0u8; BLOCK_SIZE])
        .map_err(|_| "failed to clear replayed SBFS journal")?;
    device.flush().map_err(|_| "failed to flush replayed SBFS journal")?;
    Ok(())
}

fn read_sector(device: &Arc<dyn BlockDevice>, lba: u64) -> Result<Sector, &'static str> {
    let mut sector = [0u8; BLOCK_SIZE];
    device
        .read_blocks(lba, &mut sector)
        .map_err(|error| match error {
            BlockError::OutOfRange => "SBFS block is out of range",
            _ => "SBFS block read failed",
        })?;
    Ok(sector)
}

fn get_bit(bitmap: &[u8], block: u64) -> bool {
    let byte = (block / 8) as usize;
    byte < bitmap.len() && bitmap[byte] & (1 << (block % 8)) != 0
}

fn set_bit(bitmap: &mut [u8], block: u64, allocated: bool) {
    let byte = (block / 8) as usize;
    let mask = 1 << (block % 8);
    if let Some(value) = bitmap.get_mut(byte) {
        if allocated { *value |= mask; } else { *value &= !mask; }
    }
}

fn hash_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn get_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}
fn get_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap_or([0; 4]))
}
fn get_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(input[offset..offset + 8].try_into().unwrap_or([0; 8]))
}
fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
