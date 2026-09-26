use super::tmpfs::Tmpfs;
use super::vnode::{Acl, Path, VNodeKind, VNodeMetadata};
use crate::sync::SpinLock;
use crate::volume::VolumeObject;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub trait Filesystem: Send + Sync {
    fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str>;
    fn kind(&self, id: u64) -> Option<VNodeKind>;
    fn metadata(&self, id: u64) -> Option<VNodeMetadata>;
    fn name(&self, id: u64) -> Option<alloc::string::String>;
    fn read_at(&self, id: u64, offset: usize, out: &mut [u8]) -> Result<usize, &'static str>;
    fn write_at(&self, id: u64, offset: usize, input: &[u8]) -> Result<usize, &'static str>;
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
    pub target: Path,
    pub volume: Arc<VolumeObject>,
    pub filesystem: Arc<dyn Filesystem>,
}
static MOUNTS: SpinLock<Vec<Mount>> = SpinLock::new(Vec::new());

pub struct TmpfsBackend(pub SpinLock<Tmpfs>);
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

pub struct SbfsBackend(pub SpinLock<crate::sbfs::Sbfs>);
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
        let device = root_volume.block_device.clone().ok_or("SBFS volume has no block device")?;
        let mut sbfs = crate::sbfs::Sbfs::mount_or_format(device)?;
        sbfs.initialize_system_tree(shell_image, applications)?;
        crate::kprintln!("SBFS: mounted persistent volume UUID {:02x?}", sbfs.volume_uuid());
        Arc::new(SbfsBackend(SpinLock::new(sbfs)))
    } else {
        let backend = Arc::new(TmpfsBackend(SpinLock::new(Tmpfs::empty())));
        backend.0.lock().init(shell_image, applications)?;
        backend
    };
    let mut mounts = MOUNTS.lock();
    mounts.clear();
    mounts.push(Mount {
        target: Path::new("/")?,
        volume: root_volume,
        filesystem,
    });
    Ok(())
}
pub fn fs() -> Arc<dyn Filesystem> {
    MOUNTS
        .lock()
        .iter()
        .find(|mount| mount.target.as_str() == "/")
        .map(|mount| mount.filesystem.clone())
        .expect("root filesystem is mounted")
}
pub fn executable(path: &str, cwd: &str) -> Result<Vec<u8>, &'static str> {
    fs().read_executable(path, cwd)
}
pub fn metadata(path: &str, cwd: &str) -> Result<VNodeMetadata, &'static str> {
    let filesystem = fs();
    let id = filesystem.resolve(path, cwd)?;
    filesystem.metadata(id).ok_or("file metadata not found")
}
pub fn metadata_by_id(id: u64) -> Option<VNodeMetadata> {
    fs().metadata(id)
}

pub fn create_file(path: &str, cwd: &str, executable: bool) -> Result<u64, &'static str> {
    create_path(path, cwd, VNodeKind::File, executable)
}

pub fn create_file_with_acl(
    path: &str,
    cwd: &str,
    uid: u32,
    gid: u32,
    acl: Acl,
) -> Result<u64, &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let split = absolute.rfind('/').ok_or("invalid path")?;
    let name = &absolute[split + 1..];
    if name.is_empty() { return Err("cannot create root"); }
    let parent_path = if split == 0 { "/" } else { &absolute[..split] };
    let filesystem = fs();
    let parent = filesystem.resolve(parent_path, "/")?;
    filesystem.create_file_with_acl(parent, name, uid, gid, acl)
}

pub fn create_directory(path: &str, cwd: &str) -> Result<u64, &'static str> {
    create_path(path, cwd, VNodeKind::Directory, false)
}

fn create_path(path: &str, cwd: &str, kind: VNodeKind, executable: bool) -> Result<u64, &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let split = absolute.rfind('/').ok_or("invalid path")?;
    let name = &absolute[split + 1..];
    if name.is_empty() {
        return Err("cannot create root");
    }
    let parent_path = if split == 0 { "/" } else { &absolute[..split] };
    let filesystem = fs();
    let parent = filesystem.resolve(parent_path, "/")?;
    match kind {
        VNodeKind::File => filesystem.create_file(parent, name, executable),
        VNodeKind::Directory => filesystem.create_directory(parent, name),
    }
}

pub fn parent_directory(path: &str, cwd: &str) -> Result<u64, &'static str> {
    let absolute = absolute_path(path, cwd)?;
    let split = absolute.rfind('/').ok_or("invalid path")?;
    if split + 1 == absolute.len() {
        return Err("path does not name a file");
    }
    let parent_path = if split == 0 { "/" } else { &absolute[..split] };
    fs().resolve(parent_path, "/")
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
    let filesystem = fs();
    let id = filesystem.resolve(path, cwd)?;
    filesystem.remove(id)
}

pub fn rename(old_path: &str, new_path: &str, cwd: &str) -> Result<(), &'static str> {
    let old_absolute = absolute_path(old_path, cwd)?;
    let new_absolute = absolute_path(new_path, cwd)?;
    let filesystem = fs();
    let id = filesystem.resolve(&old_absolute, "/")?;
    if id == filesystem.resolve("/", "/")? {
        return Err("cannot rename root");
    }
    if old_absolute == new_absolute {
        return Ok(());
    }
    let (new_parent_path, name) = split_parent_name(&new_absolute)?;
    let parent = filesystem.resolve(new_parent_path, "/")?;
    filesystem.rename_node(id, parent, name)
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
    let filesystem = fs();
    let id = filesystem.resolve(path, cwd)?;
    filesystem.set_owner_acl(id, uid, gid, acl)
}
pub fn mount_list() -> Vec<(alloc::string::String, alloc::string::String)> {
    MOUNTS
        .lock()
        .iter()
        .map(|mount| (mount.target.as_str().into(), mount.volume.name.clone()))
        .collect()
}
