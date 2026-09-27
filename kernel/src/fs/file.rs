use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use core::any::Any;
use core::sync::atomic::{AtomicBool, Ordering};

pub struct FileObject {
    header: ObjectHeader,
    pub node: super::vnode::NodeRef,
    pub readable: bool,
    pub writable: bool,
    append: AtomicBool,
    offset: SpinLock<usize>,
}
impl FileObject {
    pub fn new(node: super::vnode::NodeRef, readable: bool, writable: bool) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::File),
            node,
            readable,
            writable,
            append: AtomicBool::new(false),
            offset: SpinLock::new(0),
        }
    }
    /// O_APPEND: every write is placed at the current end of the file.
    pub fn set_append(&self, append: bool) {
        self.append.store(append, Ordering::Relaxed);
    }
    pub fn append(&self) -> bool {
        self.append.load(Ordering::Relaxed)
    }
    pub fn read(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        if !self.readable {
            return Err("file is not readable");
        }
        let mut off = self.offset.lock();
        let n = self.read_at(*off, out)?;
        *off += n;
        Ok(n)
    }
    pub fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, &'static str> {
        if !self.readable {
            return Err("file is not readable");
        }
        super::vfs::read_at(self.node, offset, out)
    }
    pub fn write(&self, input: &[u8]) -> Result<usize, &'static str> {
        if !self.writable {
            return Err("file is not writable");
        }
        let mut off = self.offset.lock();
        if self.append() {
            *off = super::vfs::metadata_ref(self.node).map(|meta| meta.size).unwrap_or(*off);
        }
        let n = self.write_at(*off, input)?;
        *off += n;
        Ok(n)
    }
    pub fn write_at(&self, offset: usize, input: &[u8]) -> Result<usize, &'static str> {
        if !self.writable {
            return Err("file is not writable");
        }
        super::vfs::write_at(self.node, offset, input)
    }
    pub fn seek(&self, offset: i64, whence: u64) -> Result<usize, &'static str> {
        let base = match whence {
            0 => 0usize,
            1 => *self.offset.lock(),
            2 => super::vfs::metadata_ref(self.node)
                .ok_or("file metadata not found")?
                .size,
            _ => return Err("invalid seek origin"),
        };
        let next = if offset < 0 {
            base.checked_sub(offset.unsigned_abs() as usize)
                .ok_or("negative seek position")?
        } else {
            base.checked_add(offset as usize).ok_or("seek overflow")?
        };
        *self.offset.lock() = next;
        Ok(next)
    }
}
impl KernelObject for FileObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct DirectoryObject {
    header: ObjectHeader,
    pub node: super::vnode::NodeRef,
}
impl DirectoryObject {
    pub fn new(node: super::vnode::NodeRef) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Directory),
            node,
        }
    }
}
impl KernelObject for DirectoryObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
