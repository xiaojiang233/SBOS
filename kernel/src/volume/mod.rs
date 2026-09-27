use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use crate::device::BlockDevice;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VolumeState {
    Mounted,
    Offline,
}

pub struct VolumeObject {
    header: ObjectHeader,
    pub name: String,
    pub filesystem: String,
    pub mount_point: String,
    pub state: VolumeState,
    pub block_device: Option<Arc<dyn BlockDevice>>,
}
impl VolumeObject {
    pub fn new(
        name: &str,
        filesystem: &str,
        mount_point: &str,
        block_device: Option<Arc<dyn BlockDevice>>,
    ) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Volume),
            name: String::from(name),
            filesystem: String::from(filesystem),
            mount_point: String::from(mount_point),
            state: VolumeState::Mounted,
            block_device,
        }
    }
}
impl KernelObject for VolumeObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

static VOLUMES: SpinLock<Vec<Arc<VolumeObject>>> = SpinLock::new(Vec::new());
pub fn init() -> Arc<VolumeObject> {
    let block_device = crate::device::block_devices().into_iter().next();
    let volume = if cfg!(feature = "fs-sbfs") {
        if let Some(device) = block_device {
            Arc::new(VolumeObject::new("Primary Disk", "sbfs", "/", Some(device)))
        } else {
            Arc::new(VolumeObject::new("System RAM", "tmpfs", "/", None))
        }
    } else {
        #[cfg(feature = "fs-tmpfs")]
        { Arc::new(VolumeObject::new("System RAM", "tmpfs", "/", None)) }
        #[cfg(not(feature = "fs-tmpfs"))]
        { Arc::new(VolumeObject::new("Unavailable", "sbfs", "/", None)) }
    };
    VOLUMES.lock().clear();
    VOLUMES.lock().push(volume.clone());
    volume
}
pub fn root() -> Option<Arc<VolumeObject>> {
    VOLUMES.lock().first().cloned()
}
pub fn list() -> Vec<Arc<VolumeObject>> {
    VOLUMES.lock().clone()
}
