use crate::sync::SpinLock;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverState {
    Discovered,
    Bound,
    Active,
    Failed,
}
pub struct DriverRecord {
    pub name: String,
    pub version: u32,
    pub state: DriverState,
    pub devices: Vec<u64>,
}
static DRIVERS: SpinLock<Vec<DriverRecord>> = SpinLock::new(Vec::new());
pub fn init() {
    let mut d = DRIVERS.lock();
    d.clear();
    let names = alloc::vec!["serial-16550", "uefi-gop", "i8042-keyboard"];
    #[cfg(feature = "driver-ps2-mouse")]
    let names = {
        let mut names = names;
        if crate::drivers::mouse::is_initialized() { names.push("i8042-mouse"); }
        names
    };
    for name in names {
        d.push(DriverRecord {
            name: name.into(),
            version: 1,
            state: DriverState::Active,
            devices: Vec::new(),
        });
    }
}
pub fn list() -> Vec<(String, DriverState)> {
    DRIVERS
        .lock()
        .iter()
        .map(|d| (d.name.clone(), d.state))
        .collect()
}
