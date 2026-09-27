//! Minimal PCI configuration-mechanism-1 enumerator.
//!
//! This is intentionally limited to the legacy PCI segment and bus zero
//! topology used by QEMU's `pc` machine. ACPI MCFG/ECAM and hot-plug belong to
//! the later PCI bus manager.

use crate::arch::x86_64::port::{in32, out32};

const CONFIG_ADDRESS: u16 = 0x0cf8;
const CONFIG_DATA: u16 = 0x0cfc;

#[derive(Clone, Copy, Debug)]
pub struct PciFunction {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub header_type: u8,
}

pub fn read32(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    let address = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc);
    unsafe {
        out32(CONFIG_ADDRESS, address);
        in32(CONFIG_DATA)
    }
}

pub fn write16(bus: u8, device: u8, function: u8, offset: u8, value: u16) {
    let address = 0x8000_0000
        | ((bus as u32) << 16)
        | ((device as u32) << 11)
        | ((function as u32) << 8)
        | ((offset as u32) & 0xfc);
    unsafe {
        out32(CONFIG_ADDRESS, address);
        let old = in32(CONFIG_DATA);
        let shift = (offset as u32 & 2) * 8;
        out32(CONFIG_DATA, (old & !(0xffff << shift)) | ((value as u32) << shift));
    }
}

pub fn find(vendor_id: u16, device_id: u16) -> Option<PciFunction> {
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            let id = read32(bus, device, 0, 0);
            if id as u16 == 0xffff {
                continue;
            }
            let header = read32(bus, device, 0, 0x0c);
            let header_type = ((header >> 16) & 0xff) as u8;
            let functions = if header_type & 0x80 != 0 { 8 } else { 1 };
            for function in 0..functions {
                let id = read32(bus, device, function, 0);
                let vendor = id as u16;
                let device_id_found = (id >> 16) as u16;
                if vendor == vendor_id && device_id_found == device_id {
                    let class = read32(bus, device, function, 0x08);
                    let header = read32(bus, device, function, 0x0c);
                    return Some(PciFunction {
                        bus,
                        device,
                        function,
                        vendor_id: vendor,
                        device_id: device_id_found,
                        class: (class >> 24) as u8,
                        subclass: (class >> 16) as u8,
                        header_type: (header >> 16) as u8,
                    });
                }
            }
        }
    }
    None
}
