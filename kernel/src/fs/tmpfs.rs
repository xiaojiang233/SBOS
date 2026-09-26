use super::vnode::{Acl, NodeId, VNode, VNodeKind};
use alloc::string::String;
use alloc::vec::Vec;

pub struct Tmpfs {
    nodes: Vec<VNode>,
    next_id: u64,
}

impl Tmpfs {
    pub const fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 1,
        }
    }

    pub fn init(
        &mut self,
        shell_image: &[u8],
        applications: &[(&str, &[u8])],
    ) -> Result<(), &'static str> {
        self.nodes.clear();
        self.next_id = 1;
        let root = self.add(None, "", VNodeKind::Directory, false, Vec::new())?;
        let applications_directory = self.add(
            Some(root),
            "Applications",
            VNodeKind::Directory,
            false,
            Vec::new(),
        )?;
        let system = self.add(
            Some(root),
            "System",
            VNodeKind::Directory,
            false,
            Vec::new(),
        )?;
        let users = self.add(Some(root), "Users", VNodeKind::Directory, false, Vec::new())?;
        let _shared = self.add(
            Some(root),
            "Shared",
            VNodeKind::Directory,
            false,
            Vec::new(),
        )?;
        let _volumes = self.add(
            Some(root),
            "Volumes",
            VNodeKind::Directory,
            false,
            Vec::new(),
        )?;
        self.add(Some(system), "Readme.txt", VNodeKind::File, false, b"SBOS is an object-oriented operating system.\nFiles and directories live in VFS; devices, processes, services, and configuration use native APIs.\n".to_vec())?;
        let guest = self.add(
            Some(users),
            "Guest",
            VNodeKind::Directory,
            false,
            Vec::new(),
        )?;
        self.add(
            Some(guest),
            "Welcome.txt",
            VNodeKind::File,
            false,
            b"Welcome to SBOS. Try `help` for shell commands.\n".to_vec(),
        )?;
        self.add(
            Some(applications_directory),
            "bash",
            VNodeKind::File,
            true,
            shell_image.to_vec(),
        )?;
        for (name, image) in applications {
            if image.starts_with(b"\x7fELF") {
                self.add(
                    Some(applications_directory),
                    name,
                    VNodeKind::File,
                    true,
                    image.to_vec(),
                )?;
            }
        }
        Ok(())
    }

    fn add(
        &mut self,
        parent: Option<u64>,
        name: &str,
        kind: VNodeKind,
        executable: bool,
        data: Vec<u8>,
    ) -> Result<u64, &'static str> {
        if name.len() > 64 {
            return Err("name too long");
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).ok_or("inode space exhausted")?;
        self.nodes.push(VNode {
            id,
            node_id: NodeId::tmpfs(id),
            parent,
            name: String::from(name),
            kind,
            executable,
            uid: 1000,
            gid: 1000,
            acl: Acl::OWNER_DEFAULT,
            created: 0,
            modified: 0,
            accessed: 0,
            flags: u32::from(executable),
            data,
        });
        Ok(id)
    }

    pub fn root_id(&self) -> u64 {
        self.nodes.first().map(|node| node.id).unwrap_or(0)
    }
    pub fn node(&self, id: u64) -> Option<&VNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
    pub fn node_mut(&mut self, id: u64) -> Option<&mut VNode> {
        self.nodes.iter_mut().find(|node| node.id == id)
    }
    pub fn child(&self, parent: u64, name: &str) -> Option<u64> {
        self.nodes
            .iter()
            .find(|node| node.parent == Some(parent) && node.name == name)
            .map(|node| node.id)
    }
    pub fn children(&self, parent: u64) -> impl Iterator<Item = &VNode> {
        self.nodes
            .iter()
            .filter(move |node| node.parent == Some(parent))
    }
    pub fn create_file(
        &mut self,
        parent: u64,
        name: &str,
        executable: bool,
    ) -> Result<u64, &'static str> {
        self.create_file_with_acl(parent, name, executable, 1000, 1000, Acl::OWNER_DEFAULT)
    }

    pub fn create_file_with_acl(
        &mut self,
        parent: u64,
        name: &str,
        executable: bool,
        uid: u32,
        gid: u32,
        acl: Acl,
    ) -> Result<u64, &'static str> {
        if self.node(parent).map(|n| n.kind) != Some(VNodeKind::Directory) {
            return Err("parent is not a directory");
        }
        if self.child(parent, name).is_some() {
            return Err("name already exists");
        }
        let id = self.add(Some(parent), name, VNodeKind::File, executable, Vec::new())?;
        let node = self.node_mut(id).ok_or("created file disappeared")?;
        node.uid = uid;
        node.gid = gid;
        node.acl = acl;
        Ok(id)
    }

    pub fn create_directory(&mut self, parent: u64, name: &str) -> Result<u64, &'static str> {
        if self.node(parent).map(|n| n.kind) != Some(VNodeKind::Directory) {
            return Err("parent is not a directory");
        }
        if self.child(parent, name).is_some() {
            return Err("name already exists");
        }
        self.add(Some(parent), name, VNodeKind::Directory, false, Vec::new())
    }

    pub fn remove(&mut self, id: u64) -> Result<(), &'static str> {
        let index = self.nodes.iter().position(|node| node.id == id).ok_or("file not found")?;
        if self.nodes[index].parent.is_none() {
            return Err("cannot remove root");
        }
        if self.nodes[index].kind == VNodeKind::Directory
            && self.nodes.iter().any(|node| node.parent == Some(id))
        {
            return Err("directory is not empty");
        }
        self.nodes.remove(index);
        Ok(())
    }

    pub fn rename_node(&mut self, id: u64, new_parent: u64, new_name: &str) -> Result<(), &'static str> {
        if new_name.is_empty() || new_name.len() > 64 || new_name.contains('/') {
            return Err("invalid file name");
        }
        let source_index = self.nodes.iter().position(|node| node.id == id).ok_or("file not found")?;
        if self.nodes[source_index].parent.is_none() {
            return Err("cannot rename root");
        }
        if self.node(new_parent).map(|node| node.kind) != Some(VNodeKind::Directory) {
            return Err("not a directory");
        }
        if self.nodes[source_index].kind == VNodeKind::Directory {
            let mut cursor = Some(new_parent);
            while let Some(ancestor) = cursor {
                if ancestor == id {
                    return Err("directory move would create a cycle");
                }
                cursor = self.node(ancestor).and_then(|node| node.parent);
            }
        }
        let target = self.child(new_parent, new_name);
        if target == Some(id) {
            return Ok(());
        }
        if let Some(target_id) = target {
            let target_index = self.nodes.iter().position(|node| node.id == target_id).ok_or("file not found")?;
            if self.nodes[target_index].kind != self.nodes[source_index].kind {
                return Err("rename type mismatch");
            }
            if self.nodes[target_index].kind == VNodeKind::Directory
                && self.nodes.iter().any(|node| node.parent == Some(target_id))
            {
                return Err("directory is not empty");
            }
            self.nodes.remove(target_index);
        }
        let source = self.nodes.iter_mut().find(|node| node.id == id).ok_or("file not found")?;
        source.parent = Some(new_parent);
        source.name = String::from(new_name);
        source.modified = 0;
        Ok(())
    }

    pub fn resolve(&self, path: &str, cwd: &str) -> Result<u64, &'static str> {
        if path.len() > 512 || cwd.len() > 512 {
            return Err("path too long");
        }
        if path.starts_with('/') {
            return self.resolve_absolute(path);
        }
        let relative = if path.is_empty() { "." } else { path };
        let mut joined = [0u8; 1024];
        let cwd_bytes = cwd.as_bytes();
        let path_bytes = relative.as_bytes();
        let separator = if cwd == "/" { 0 } else { 1 };
        let need = cwd_bytes.len() + separator + path_bytes.len();
        if need > joined.len() {
            return Err("path too long");
        }
        joined[..cwd_bytes.len()].copy_from_slice(cwd_bytes);
        if separator != 0 {
            joined[cwd_bytes.len()] = b'/';
        }
        joined[cwd_bytes.len() + separator..need].copy_from_slice(path_bytes);
        self.resolve_absolute(
            core::str::from_utf8(&joined[..need]).map_err(|_| "invalid utf-8 path")?,
        )
    }

    fn resolve_absolute(&self, path: &str) -> Result<u64, &'static str> {
        let mut current = self.root_id();
        for part in path.split('/') {
            if part.is_empty() || part == "." {
                continue;
            }
            if part == ".." {
                current = self
                    .node(current)
                    .and_then(|n| n.parent)
                    .unwrap_or(self.root_id());
                continue;
            }
            current = self.child(current, part).ok_or("path not found")?;
        }
        Ok(current)
    }

    pub fn read_at(&self, id: u64, offset: usize, out: &mut [u8]) -> Result<usize, &'static str> {
        let node = self.node(id).ok_or("file not found")?;
        if node.kind != VNodeKind::File {
            return Err("not a file");
        }
        let count = out.len().min(node.data.len().saturating_sub(offset));
        out[..count].copy_from_slice(&node.data[offset..offset + count]);
        Ok(count)
    }
    pub fn write_at(
        &mut self,
        id: u64,
        offset: usize,
        input: &[u8],
    ) -> Result<usize, &'static str> {
        let node = self.node_mut(id).ok_or("file not found")?;
        if node.kind != VNodeKind::File {
            return Err("not a file");
        }
        let end = offset.checked_add(input.len()).ok_or("file too large")?;
        if end > 1024 * 1024 {
            return Err("tmpfs file limit exceeded");
        }
        if node.data.len() < end {
            node.data
                .try_reserve(end - node.data.len())
                .map_err(|_| "tmpfs is out of memory")?;
            node.data.resize(end, 0);
        }
        node.data[offset..end].copy_from_slice(input);
        Ok(input.len())
    }

    pub fn truncate(&mut self, id: u64) -> Result<(), &'static str> {
        let node = self.node_mut(id).ok_or("file not found")?;
        if node.kind != VNodeKind::File { return Err("not a file"); }
        node.data.clear();
        node.modified = 0;
        Ok(())
    }
}
