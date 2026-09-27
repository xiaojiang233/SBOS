use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceClass {
    Display,
    Input,
    Serial,
    Storage,
    Network,
    Audio,
    Usb,
    Pci,
    Camera,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceState {
    Discovered,
    Initialized,
    Online,
    Failed,
    Removed,
}

#[derive(Clone, Copy, Debug)]
pub struct DeviceCapabilities(pub u64);
impl DeviceCapabilities {
    pub const DISPLAY: Self = Self(1);
    pub const INPUT: Self = Self(2);
    pub const SERIAL_IO: Self = Self(4);
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

pub struct DeviceObject {
    header: ObjectHeader,
    pub id: DeviceId,
    pub class: DeviceClass,
    pub vendor: String,
    pub model: String,
    pub driver: String,
    pub state: DeviceState,
    pub capabilities: DeviceCapabilities,
    pub parent: Option<DeviceId>,
    pub children: Vec<DeviceId>,
}
impl DeviceObject {
    fn new(
        id: u64,
        class: DeviceClass,
        vendor: &str,
        model: &str,
        driver: &str,
        caps: DeviceCapabilities,
    ) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Device),
            id: DeviceId(id),
            class,
            vendor: vendor.into(),
            model: model.into(),
            driver: driver.into(),
            state: DeviceState::Online,
            capabilities: caps,
            parent: None,
            children: Vec::new(),
        }
    }
}
impl KernelObject for DeviceObject {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait DisplayDevice: Send + Sync {
    fn dimensions(&self) -> (u32, u32);
}
pub trait InputDevice: Send + Sync {
    fn read_key(&self) -> u8;
}
pub trait BlockDevice: Send + Sync {
    fn block_size(&self) -> u32;
    fn block_count(&self) -> u64;
    fn read_blocks(&self, lba: u64, output: &mut [u8]) -> Result<(), BlockError>;
    fn write_blocks(&self, lba: u64, input: &[u8]) -> Result<(), BlockError>;
    fn flush(&self) -> Result<(), BlockError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockError {
    InvalidBuffer,
    OutOfRange,
    NotReady,
    Io,
}
pub trait NetworkDevice: Send + Sync {
    fn mac_address(&self) -> [u8; 6];
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError>;
    fn receive(&mut self, frame: &mut [u8]) -> Result<Option<usize>, NetworkError>;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkError {
    InvalidFrame,
    NotReady,
    Busy,
    Io,
}
pub trait AudioDevice: Send + Sync {
    fn sample_rate(&self) -> u32;
}

static DEVICES: SpinLock<Vec<Arc<DeviceObject>>> = SpinLock::new(Vec::new());
static BLOCK_DEVICES: SpinLock<Vec<Arc<dyn BlockDevice>>> = SpinLock::new(Vec::new());
pub fn init(has_framebuffer: bool) {
    let mut devices = DEVICES.lock();
    devices.clear();
    BLOCK_DEVICES.lock().clear();
    devices.push(Arc::new(DeviceObject::new(
        1,
        DeviceClass::Serial,
        "PC",
        "16550-compatible COM1",
        "serial-16550",
        DeviceCapabilities::SERIAL_IO,
    )));
    if has_framebuffer {
        devices.push(Arc::new(DeviceObject::new(
            2,
            DeviceClass::Display,
            "UEFI",
            "Graphics Output Protocol framebuffer",
            "uefi-gop",
            DeviceCapabilities::DISPLAY,
        )));
    }
    devices.push(Arc::new(DeviceObject::new(
        3,
        DeviceClass::Input,
        "PC",
        "PS/2 keyboard controller",
        "i8042-keyboard",
        DeviceCapabilities::INPUT,
    )));
}

pub fn register_block_device(
    vendor: &str,
    model: &str,
    driver: &str,
    device: Arc<dyn BlockDevice>,
) {
    let mut devices = DEVICES.lock();
    let id = devices.iter().map(|device| device.id.0).max().unwrap_or(0) + 1;
    devices.push(Arc::new(DeviceObject::new(
        id,
        DeviceClass::Storage,
        vendor,
        model,
        driver,
        DeviceCapabilities(8),
    )));
    BLOCK_DEVICES.lock().push(device);
}

pub fn register_network_device(vendor: &str, model: &str, driver: &str) {
    let mut devices = DEVICES.lock();
    let id = devices.iter().map(|device| device.id.0).max().unwrap_or(0) + 1;
    devices.push(Arc::new(DeviceObject::new(
        id,
        DeviceClass::Network,
        vendor,
        model,
        driver,
        DeviceCapabilities(16),
    )));
}

pub fn register_input_device(vendor: &str, model: &str, driver: &str) {
    let mut devices = DEVICES.lock();
    let id = devices.iter().map(|device| device.id.0).max().unwrap_or(0) + 1;
    devices.push(Arc::new(DeviceObject::new(
        id,
        DeviceClass::Input,
        vendor,
        model,
        driver,
        DeviceCapabilities::INPUT,
    )));
}

pub fn block_devices() -> Vec<Arc<dyn BlockDevice>> {
    BLOCK_DEVICES.lock().clone()
}

pub fn list() -> Vec<Arc<DeviceObject>> {
    DEVICES.lock().clone()
}
