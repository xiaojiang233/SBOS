use core::any::Any;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ObjectType {
    Process,
    Thread,
    File,
    Directory,
    Device,
    Volume,
    Channel,
    PipeReader,
    PipeWriter,
    Event,
    Timer,
    SharedMemory,
    Service,
    ConfigTransaction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObjectId(pub u64);

pub struct ObjectHeader {
    id: ObjectId,
    object_type: ObjectType,
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl ObjectHeader {
    pub fn new(object_type: ObjectType) -> Self {
        Self {
            id: ObjectId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            object_type,
        }
    }
    pub const fn id(&self) -> ObjectId {
        self.id
    }
    pub const fn object_type(&self) -> ObjectType {
        self.object_type
    }
}

pub trait KernelObject: Any + Send + Sync {
    fn header(&self) -> &ObjectHeader;
    fn as_any(&self) -> &dyn Any;
}

pub struct EventObject {
    header: ObjectHeader,
    signaled: core::sync::atomic::AtomicBool,
}
impl EventObject {
    pub fn new() -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Event),
            signaled: core::sync::atomic::AtomicBool::new(false),
        }
    }
    pub fn signal(&self) {
        self.signaled.store(true, Ordering::Release);
    }
    pub fn reset(&self) {
        self.signaled.store(false, Ordering::Release);
    }
    pub fn is_signaled(&self) -> bool {
        self.signaled.load(Ordering::Acquire)
    }
}
impl KernelObject for EventObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct TimerObject {
    header: ObjectHeader,
    deadline: core::sync::atomic::AtomicU64,
}
impl TimerObject {
    pub fn new() -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Timer),
            deadline: core::sync::atomic::AtomicU64::new(0),
        }
    }
    pub fn arm(&self, tick: u64) {
        self.deadline.store(tick, Ordering::Release);
    }
    pub fn deadline(&self) -> u64 {
        self.deadline.load(Ordering::Acquire)
    }
}
impl KernelObject for TimerObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct SharedMemoryObject {
    header: ObjectHeader,
    base: u64,
    length: usize,
}
impl SharedMemoryObject {
    pub fn new(base: u64, length: usize) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::SharedMemory),
            base,
            length,
        }
    }
    pub const fn base(&self) -> u64 {
        self.base
    }
    pub const fn length(&self) -> usize {
        self.length
    }
}
impl KernelObject for SharedMemoryObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
