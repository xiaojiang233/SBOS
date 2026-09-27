#[cfg(feature = "fs-tmpfs")]
use super::tmpfs::Tmpfs;
use super::vnode::{Acl, MountId, NodeRef, Path, VNodeKind, VNodeMetadata};
use crate::sync::SpinLock;
use crate::volume::VolumeObject;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

pub trait Filesystem: Send + Sync {
    fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str>;
    fn kind(&self, id: u64) -> Option<VNodeKind>;
    fn metadata(&self, id: u64) -> Option<VNodeMetadata>;
    fn name(&self, id: u64) -> Option<alloc::string::String>;
    fn read_at(&self, id: u64, offset: usize, out: &mut [u8]) -> Result<usize, &'static str>;
    fn write_at(&self, id: u64, offset: usize, input: &[u8]) -> Result<usize, &'static str>;
    fn sync(&self) -> Result<(), &'static str>;
    fn read_directory(&self, id: u64, out: &mut [u8]) -> Result<usize, &'static str>;
    fn create_file(&self, parent: u64, name: &str, executable: bool) -> Result<u64, &'static str>;
    fn create_file_with_acl(&self, parent: u64, name: &str, uid: u32, gid: u32, acl: Acl) -> Result<u64, &'static str>;
    fn create_directory(&self, parent: u64, name: &str) -> Result<u64, &'static str>;
    fn rename_node(&self, id: u64, parent: u64, name: &str) -> Result<(), &'static str>;
    fn remove(&self, id: u64) -> Result<(), &'static str>;
    fn truncate(&self, id: u64) -> Result<(), &'static str>;
    fn can_access(&self, id: u64, uid: u32, gid: u32, access: u16) -> bool;
    fn set_owner_acl(&self, id: u64, uid: u32, gid: u32, acl: Acl) -> Result<(), &'static str>;
    fn change_directory(
        &self,
        current: &str,
        path: &str,
    ) -> Result<alloc::string::String, &'static str>;
    fn read_executable(&self, path: &str, cwd: &str) -> Result<Vec<u8>, &'static str>;
}

pub struct Mount {
    pub id: MountId,
    pub target: Path,
    pub volume: Arc<VolumeObject>,
    pub filesystem: Arc<dyn Filesystem>,
}
static MOUNTS: SpinLock<Vec<Mount>> = SpinLock::new(Vec::new());
static NEXT_MOUNT_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(feature = "fs-tmpfs")]
pub struct TmpfsBackend(pub SpinLock<Tmpfs>);
#[cfg(feature = "fs-tmpfs")]
impl Filesystem for TmpfsBackend {
    fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str> {
        self.0.lock().resolve(path, cwd)
    }
    fn kind(&self, id: u64) -> Option<VNodeKind> {
        self.0.lock().node(id).map(|n| n.kind)
    }
    fn metadata(&self, id: u64) -> Option<VNodeMetadata> {
        self.0.lock().node(id).map(|node| VNodeMetadata {
            id: node.id,
            node_id: node.node_id,
            kind: node.kind,
            size: node.data.len(),
            executable: node.executable,
            uid: node.uid,
            gid: node.gid,
            acl: node.acl,
            created: node.created,
            modified: node.modified,
            accessed: node.accessed,
            flags: node.flags,
        })
    }
    fn name(&self, id: u64) -> Option<alloc::string::String> {
        self.0.lock().node(id).map(|n| n.name.clone())
    }
    fn read_at(&self, id: u64, offset: usize, out: &mut [u8]) -> Result<usize, &'static str> {
        self.0.lock().read_at(id, offset, out)
    }
    fn write_at(&self, id: u64, offset: usize, input: &[u8]) -> Result<usize, &'static str> {
        self.0.lock().write_at(id, offset, input)
    }
    fn sync(&self) -> Result<(), &'static str> { Ok(()) }
    fn read_directory(&self, id: u64, out: &mut [u8]) -> Result<usize, &'static str> {
        let fs = self.0.lock();
        if fs.node(id).map(|n| n.kind) != Some(VNodeKind::Directory) {
            return Err("not a directory");
        }
        let mut used = 0;
        for child in fs.children(id) {
            let needed = child.name.len() + usize::from(child.kind == VNodeKind::Directory) + 1;
            if used + needed > out.len() {
                break;
            }
            out[used..used + child.name.len()].copy_from_slice(child.name.as_bytes());
            used += child.name.len();
            if child.kind == VNodeKind::Directory {
                out[used] = b'/';
                used += 1;
            }
            out[used] = b'\n';
            used += 1;
        }
        Ok(used)
    }
    fn create_file(&self, parent: u64, name: &str, executable: bool) -> Result<u64, &'static str> {
        self.0.lock().create_file(parent, name, executable)
    }
    fn create_directory(&self, parent: u64, name: &str) -> Result<u64, &'static str> {
        self.0.lock().create_directory(parent, name)
    }
    fn rename_node(&self, id: u64, parent: u64, name: &str) -> Result<(), &'static str> {
        self.0.lock().rename_node(id, parent, name)
    }
    fn create_file_with_acl(&self, parent: u64, name: &str, uid: u32, gid: u32, acl: Acl) -> Result<u64, &'static str> {
        self.0.lock().create_file_with_acl(parent, name, false, uid, gid, acl)
    }
    fn remove(&self, id: u64) -> Result<(), &'static str> {
        self.0.lock().remove(id)
    }
    fn truncate(&self, id: u64) -> Result<(), &'static str> {
        self.0.lock().truncate(id)
    }
    fn can_access(&self, id: u64, uid: u32, gid: u32, access: u16) -> bool {
        let fs = self.0.lock();
        let Some(node) = fs.node(id) else { return false };
        node.acl.allows(node.uid, node.gid, uid, gid, access)
    }
    fn set_owner_acl(&self, id: u64, uid: u32, gid: u32, acl: Acl) -> Result<(), &'static str> {
        let mut fs = self.0.lock();
        let node = fs.node_mut(id).ok_or("file not found")?;
        node.uid = uid;
        node.gid = gid;
        node.acl = acl;
        Ok(())
    }
    fn change_directory(
        &self,
        current: &str,
        path: &str,
    ) -> Result<alloc::string::String, &'static str> {
        let fs = self.0.lock();
        let id = fs.resolve(path, current)?;
        if fs.node(id).map(|n| n.kind) != Some(VNodeKind::Directory) {
            return Err("not a directory");
        }
        if path.starts_with('/') {
            return Ok(normalize_absolute(path));
        }
        if path == "." {
            return Ok(current.into());
        }
        let mut joined = alloc::string::String::from(current);
        if !joined.ends_with('/') {
            joined.push('/');
        }
        joined.push_str(path);
        Ok(normalize_absolute(&joined))
    }
    fn read_executable(&self, path: &str, cwd: &str) -> Result<Vec<u8>, &'static str> {
        let fs = self.0.lock();
        let id = fs.resolve(path, cwd)?;
        let node = fs.node(id).ok_or("file not found")?;
        if node.kind != VNodeKind::File || !node.executable {
            return Err("not an executable file");
        }
        Ok(node.data.clone())
    }
}

#[cfg(feature = "fs-sbfs")]
pub struct SbfsBackend(pub SpinLock<crate::sbfs::Sbfs>);
#[cfg(feature = "fs-sbfs")]
impl Filesystem for SbfsBackend {
    fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str> {
        self.0.lock().resolve(path, cwd)
    }
    fn kind(&self, id: u64) -> Option<VNodeKind> {
        self.0.lock().metadata(id).map(|metadata| metadata.kind)
    }
    fn metadata(&self, id: u64) -> Option<VNodeMetadata> {
        self.0.lock().metadata(id)
    }
    fn name(&self, id: u64) -> Option<alloc::string::String> {
        self.0.lock().name(id)
    }
    fn read_at(&self, id: u64, offset: usize, out: &mut [u8]) -> Result<usize, &'static str> {
        self.0.lock().read_at(id, offset, out)
    }
    fn write_at(&self, id: u64, offset: usize, input: &[u8]) -> Result<usize, &'static str> {
        self.0.lock().write_at(id, offset, input)
    }
    fn sync(&self) -> Result<(), &'static str> { self.0.lock().sync() }
    fn read_directory(&self, id: u64, out: &mut [u8]) -> Result<usize, &'static str> {
        self.0.lock().read_directory(id, out)
    }
    fn create_file(&self, parent: u64, name: &str, executable: bool) -> Result<u64, &'static str> {
        self.0.lock().create_file(parent, name, executable)
    }
    fn create_directory(&self, parent: u64, name: &str) -> Result<u64, &'static str> {
        self.0.lock().create_directory(parent, name)
    }
    fn rename_node(&self, id: u64, parent: u64, name: &str) -> Result<(), &'static str> {
        self.0.lock().rename_node(id, parent, name)
    }
    fn create_file_with_acl(&self, parent: u64, name: &str, uid: u32, gid: u32, acl: Acl) -> Result<u64, &'static str> {
        self.0.lock().create_file_with_acl(parent, name, uid, gid, acl)
    }
    fn remove(&self, id: u64) -> Result<(), &'static str> {
        self.0.lock().remove(id)
    }
    fn truncate(&self, id: u64) -> Result<(), &'static str> {
        self.0.lock().truncate(id)
    }
    fn can_access(&self, id: u64, uid: u32, gid: u32, access: u16) -> bool {
        self.0.lock().can_access(id, uid, gid, access)
    }
    fn set_owner_acl(&self, id: u64, uid: u32, gid: u32, acl: Acl) -> Result<(), &'static str> {
        self.0.lock().set_owner_acl(id, uid, gid, acl)
    }
    fn change_directory(&self, current: &str, path: &str) -> Result<alloc::string::String, &'static str> {
        let fs = self.0.lock();
        let id = fs.resolve(path, current)?;
        if fs.metadata(id).map(|node| node.kind) != Some(VNodeKind::Directory) {
            return Err("not a directory");
        }
        if path.starts_with('/') {
            return Ok(normalize_absolute(path));
        }
        let mut joined = alloc::string::String::from(current);
        if !joined.ends_with('/') { joined.push('/'); }
        joined.push_str(path);
        Ok(normalize_absolute(&joined))
    }
    fn read_executable(&self, path: &str, cwd: &str) -> Result<Vec<u8>, &'static str> {
        self.0.lock().read_executable(path, cwd)
    }
}

fn normalize_absolute(path: &str) -> alloc::string::String {
    let mut parts: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop();
        } else {
            parts.push(part);
        }
    }
    let mut result = alloc::string::String::from("/");
    for (index, part) in parts.iter().enumerate() {
        if index != 0 {
            result.push('/');
        }
        result.push_str(part);
    }
    result
}

pub fn init(
    shell_image: &[u8],
    applications: &[(&str, &[u8])],
    root_volume: Arc<VolumeObject>,
) -> Result<(), &'static str> {
    let filesystem: Arc<dyn Filesystem> = if root_volume.filesystem == "sbfs" {
        #[cfg(feature = "fs-sbfs")]
        {
            let device = root_volume.block_device.clone().ok_or("SBFS volume has no block device")?;
            let mut sbfs = crate::sbfs::Sbfs::mount_or_format(device)?;
            sbfs.initialize_system_tree(shell_image, applications)?;
            crate::kprintln!("SBFS: mounted persistent volume UUID {:02x?}", sbfs.volume_uuid());
            Arc::new(SbfsBackend(SpinLock::new(sbfs)))
        }
        #[cfg(not(feature = "fs-sbfs"))]
        { return Err("root volume requires the disabled fs-sbfs feature"); }
    } else {
        #[cfg(feature = "fs-tmpfs")]
        {
            let backend = Arc::new(TmpfsBackend(SpinLock::new(Tmpfs::empty())));
            backend.0.lock().init(shell_image, applications)?;
            backend
        }
        #[cfg(not(feature = "fs-tmpfs"))]
        { return Err("root volume requires the disabled fs-tmpfs feature"); }
    };
    MOUNTS.lock_irqsave().clear();
    mount("/", root_volume, filesystem)?;
    Ok(())
}

/// Attach a filesystem backend at a virtual path. Path lookup chooses the
/// deepest matching mount; open objects retain the returned MountId.
pub fn mount(
    target: &str,
    volume: Arc<VolumeObject>,
    filesystem: Arc<dyn Filesystem>,
) -> Result<MountId, &'static str> {
    let target = Path::new(&normalize_absolute(target))?;
    let mut mounts = MOUNTS.lock_irqsave();
    if mounts.iter().any(|mount| mount.target.as_str() == target.as_str()) {
        return Err("mount point is already occupied");
    }
    let id = MountId(NEXT_MOUNT_ID.fetch_add(1, Ordering::Relaxed));
    mounts.push(Mount { id, target, volume, filesystem });
    Ok(id)
}
pub fn fs() -> Arc<dyn Filesystem> {
    MOUNTS
        .lock()
        .iter()
        .find(|mount| mount.target.as_str() == "/")
        .map(|mount| mount.filesystem.clone())
        .expect("root filesystem is mounted")
}

/// Resolve a path against the most specific mount and retain both parts of its
/// identity. This is the only constructor open-object code should use.
pub fn lookup(path: &str, cwd: &str) -> Result<(NodeRef, Arc<dyn Filesystem>), &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let mounts = MOUNTS.lock_irqsave();
    let mount = mounts
        .iter()
        .filter(|mount| {
            let target = mount.target.as_str();
            target == "/"
                || absolute == target
                || absolute.strip_prefix(target).is_some_and(|tail| tail.starts_with('/'))
        })
        .max_by_key(|mount| mount.target.as_str().len())
        .ok_or("no filesystem mounted for path")?;
    let target = mount.target.as_str();
    let relative = if target == "/" {
        absolute.as_str()
    } else if absolute == target {
        "/"
    } else {
        &absolute[target.len()..]
    };
    let node_id = mount.filesystem.resolve(relative, "/")?;
    Ok((NodeRef { mount_id: mount.id, node_id }, mount.filesystem.clone()))
}

pub fn filesystem_for(node: NodeRef) -> Result<Arc<dyn Filesystem>, &'static str> {
    MOUNTS
        .lock()
        .iter()
        .find(|mount| mount.id == node.mount_id)
        .map(|mount| mount.filesystem.clone())
        .ok_or("mount no longer exists")
}

pub fn kind_ref(node: NodeRef) -> Option<VNodeKind> {
    filesystem_for(node).ok()?.kind(node.node_id)
}

pub fn metadata_ref(node: NodeRef) -> Option<VNodeMetadata> {
    filesystem_for(node).ok()?.metadata(node.node_id)
}

pub fn read_at(node: NodeRef, offset: usize, output: &mut [u8]) -> Result<usize, &'static str> {
    filesystem_for(node)?.read_at(node.node_id, offset, output)
}

pub fn write_at(node: NodeRef, offset: usize, input: &[u8]) -> Result<usize, &'static str> {
    filesystem_for(node)?.write_at(node.node_id, offset, input)
}

pub fn sync_node(node: NodeRef) -> Result<(), &'static str> {
    filesystem_for(node)?.sync()
}

pub fn read_directory(node: NodeRef, output: &mut [u8]) -> Result<usize, &'static str> {
    filesystem_for(node)?.read_directory(node.node_id, output)
}

pub fn executable(path: &str, cwd: &str) -> Result<Vec<u8>, &'static str> {
    let (node, filesystem) = lookup(path, cwd)?;
    let metadata = filesystem.metadata(node.node_id).ok_or("file metadata not found")?;
    if metadata.kind != VNodeKind::File || !metadata.executable {
        return Err("not an executable file");
    }
    let mut image = alloc::vec![0; metadata.size];
    let read = filesystem.read_at(node.node_id, 0, &mut image)?;
    image.truncate(read);
    Ok(image)
}
pub fn metadata(path: &str, cwd: &str) -> Result<VNodeMetadata, &'static str> {
    let (node, filesystem) = lookup(path, cwd)?;
    filesystem.metadata(node.node_id).ok_or("file metadata not found")
}

fn parent_ref(path: &str, cwd: &str) -> Result<(NodeRef, Arc<dyn Filesystem>, alloc::string::String), &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let (parent_path, name) = split_parent_name(&absolute)?;
    let (parent, filesystem) = lookup(parent_path, "/")?;
    Ok((parent, filesystem, alloc::string::String::from(name)))
}

pub fn parent_node(path: &str, cwd: &str) -> Result<(NodeRef, Arc<dyn Filesystem>), &'static str> {
    let (node, filesystem, _) = parent_ref(path, cwd)?;
    Ok((node, filesystem))
}

pub fn change_directory(cwd: &str, path: &str) -> Result<alloc::string::String, &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let (node, filesystem) = lookup(&absolute, "/")?;
    if filesystem.kind(node.node_id) != Some(VNodeKind::Directory) {
        return Err("not a directory");
    }
    Ok(absolute)
}
pub fn create_file(path: &str, cwd: &str, executable: bool) -> Result<NodeRef, &'static str> {
    create_path(path, cwd, VNodeKind::File, executable)
}

pub fn create_file_with_acl(
    path: &str,
    cwd: &str,
    uid: u32,
    gid: u32,
    acl: Acl,
) -> Result<NodeRef, &'static str> {
    let (parent, filesystem, name) = parent_ref(path, cwd)?;
    let node_id = filesystem.create_file_with_acl(parent.node_id, &name, uid, gid, acl)?;
    Ok(NodeRef { mount_id: parent.mount_id, node_id })
}

pub fn create_directory(path: &str, cwd: &str) -> Result<NodeRef, &'static str> {
    create_path(path, cwd, VNodeKind::Directory, false)
}

fn create_path(path: &str, cwd: &str, kind: VNodeKind, executable: bool) -> Result<NodeRef, &'static str> {
    let (parent, filesystem, name) = parent_ref(path, cwd)?;
    let node_id = match kind {
        VNodeKind::File => filesystem.create_file(parent.node_id, &name, executable),
        VNodeKind::Directory => filesystem.create_directory(parent.node_id, &name),
    }?;
    Ok(NodeRef { mount_id: parent.mount_id, node_id })
}

pub fn parent_directory(path: &str, cwd: &str) -> Result<NodeRef, &'static str> {
    parent_node(path, cwd).map(|(node, _filesystem)| node)
}

fn absolute_path(path: &str, cwd: &str) -> Result<alloc::string::String, &'static str> {
    if path.is_empty() || path.len() > 512 || cwd.len() > 512 {
        return Err("invalid path");
    }
    if path.starts_with('/') {
        Ok(normalize_absolute(path))
    } else {
        let mut joined = alloc::string::String::from(cwd);
        if !joined.ends_with('/') { joined.push('/'); }
        joined.push_str(path);
        Ok(normalize_absolute(&joined))
    }
}

pub fn remove(path: &str, cwd: &str) -> Result<(), &'static str> {
    let (node, filesystem) = lookup(path, cwd)?;
    filesystem.remove(node.node_id)
}

pub fn rename(old_path: &str, new_path: &str, cwd: &str) -> Result<(), &'static str> {
    let old_absolute = absolute_path(old_path, cwd)?;
    let new_absolute = absolute_path(new_path, cwd)?;
    let (source, filesystem) = lookup(&old_absolute, "/")?;
    if source.node_id == filesystem.resolve("/", "/")? {
        return Err("cannot rename root");
    }
    if old_absolute == new_absolute {
        return Ok(());
    }
    let (new_parent_path, name) = split_parent_name(&new_absolute)?;
    let (parent, destination_fs) = lookup(new_parent_path, "/")?;
    if source.mount_id != parent.mount_id {
        return Err("cross-device rename");
    }
    let _ = destination_fs;
    filesystem.rename_node(source.node_id, parent.node_id, name)
}

fn split_parent_name(path: &str) -> Result<(&str, &str), &'static str> {
    let slash = path.rfind('/').ok_or("invalid path")?;
    let name = &path[slash + 1..];
    if name.is_empty() || name == "." || name == ".." {
        return Err("invalid file name");
    }
    let parent = if slash == 0 { "/" } else { &path[..slash] };
    Ok((parent, name))
}

pub fn set_owner_acl(path: &str, cwd: &str, uid: u32, gid: u32, acl: Acl) -> Result<(), &'static str> {
    let (node, filesystem) = lookup(path, cwd)?;
    filesystem.set_owner_acl(node.node_id, uid, gid, acl)
}
pub fn mount_list() -> Vec<(alloc::string::String, alloc::string::String)> {
    MOUNTS
        .lock()
        .iter()
        .map(|mount| (mount.target.as_str().into(), mount.volume.name.clone()))
        .collect()
}
