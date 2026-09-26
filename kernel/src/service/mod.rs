use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceState {
    Starting,
    Running,
    Stopped,
    Failed,
}
pub struct ServiceObject {
    header: ObjectHeader,
    pub name: String,
    pub state: ServiceState,
    pub version: u32,
}
impl ServiceObject {
    fn new(name: &str) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Service),
            name: name.into(),
            state: ServiceState::Running,
            version: 1,
        }
    }
}
impl KernelObject for ServiceObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
static SERVICES: SpinLock<Vec<Arc<ServiceObject>>> = SpinLock::new(Vec::new());
pub fn init() {
    let mut items = SERVICES.lock();
    items.clear();
    for name in [
        "console",
        "configuration",
        "device-manager",
        "filesystem",
        "ipc",
        "object-manager",
        "process-manager",
    ] {
        items.push(Arc::new(ServiceObject::new(name)));
    }
}
pub fn list() -> Vec<Arc<ServiceObject>> {
    SERVICES.lock().clone()
}
pub fn lookup(name: &str) -> Option<Arc<ServiceObject>> {
    SERVICES.lock().iter().find(|s| s.name == name).cloned()
}
