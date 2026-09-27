//! Polling Intel 82540EM-compatible Ethernet controller (QEMU `e1000`).
//!
//! DMA rings and buffers are kept in PMM pages below the current 1 GiB direct
//! map. Interrupt-driven NAPI-style polling and DMA mapping above that limit
//! are future work.

use crate::device::{NetworkDevice, NetworkError};
use crate::drivers::pci;
use crate::memory::{pmm, vmm};
use crate::sync::SpinLock;
use core::ptr::{read_volatile, write_volatile};

const RX_COUNT: usize = 32;
const TX_COUNT: usize = 32;
const BUFFER_SIZE: usize = 2048;
const RING_BYTES: usize = RX_COUNT * 16;
const RX_BUFFERS_PAGES: usize = RX_COUNT * BUFFER_SIZE / pmm::PAGE_SIZE as usize;
const TX_BUFFERS_PAGES: usize = TX_COUNT * BUFFER_SIZE / pmm::PAGE_SIZE as usize;

const REG_CTRL: usize = 0x0000;
const REG_EERD: usize = 0x0014;
const REG_IMC: usize = 0x00d8;
const REG_RCTL: usize = 0x0100;
const REG_TCTL: usize = 0x0400;
const REG_TIPG: usize = 0x0410;
const REG_RDBAL: usize = 0x2800;
const REG_RDBAH: usize = 0x2804;
const REG_RDLEN: usize = 0x2808;
const REG_RDH: usize = 0x2810;
const REG_RDT: usize = 0x2818;
const REG_TDBAL: usize = 0x3800;
const REG_TDBAH: usize = 0x3804;
const REG_TDLEN: usize = 0x3808;
const REG_TDH: usize = 0x3810;
const REG_TDT: usize = 0x3818;
const REG_RAL: usize = 0x5400;
const REG_RAH: usize = 0x5404;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct RxDescriptor {
    address: u64,
    length: u16,
    checksum: u16,
    status: u8,
    errors: u8,
    special: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct TxDescriptor {
    address: u64,
    length: u16,
    checksum_offset: u8,
    command: u8,
    status: u8,
    checksum_start: u8,
    special: u16,
}

struct E1000 {
    mmio: usize,
    mac: [u8; 6],
    rx_ring: u64,
    tx_ring: u64,
    rx_buffers: u64,
    tx_buffers: u64,
    rx_next: usize,
    tx_next: usize,
}

static DEVICE: SpinLock<Option<E1000>> = SpinLock::new(None);

impl E1000 {
    unsafe fn read(&self, register: usize) -> u32 {
        read_volatile((self.mmio + register) as *const u32)
    }
    unsafe fn write(&self, register: usize, value: u32) {
        write_volatile((self.mmio + register) as *mut u32, value);
    }

    fn initialize() -> Result<Self, &'static str> {
        let pci = pci::find(0x8086, 0x100e).ok_or("QEMU 82540EM PCI device not found")?;
        let bar0 = pci::read32(pci.bus, pci.device, pci.function, 0x10);
        if bar0 & 1 != 0 || (bar0 >> 1) & 3 == 2 {
            return Err("unsupported E1000 BAR0 type");
        }
        let physical = (bar0 & !0xf) as u64;
        if physical == 0 {
            return Err("E1000 has no MMIO BAR0");
        }
        let command = pci::read32(pci.bus, pci.device, pci.function, 0x04) as u16;
        pci::write16(pci.bus, pci.device, pci.function, 0x04, command | 0x0006);
        let mmio = vmm::map_mmio(physical, 0x6000, 0)? as usize;

        let rx_ring = pmm::allocate_contiguous_pages(1).ok_or("E1000 RX ring allocation failed")?;
        let tx_ring = pmm::allocate_contiguous_pages(1).ok_or("E1000 TX ring allocation failed")?;
        let rx_buffers = pmm::allocate_contiguous_pages(RX_BUFFERS_PAGES)
            .ok_or("E1000 RX buffers allocation failed")?;
        let tx_buffers = pmm::allocate_contiguous_pages(TX_BUFFERS_PAGES)
            .ok_or("E1000 TX buffers allocation failed")?;
        unsafe {
            core::ptr::write_bytes(rx_ring as *mut u8, 0, pmm::PAGE_SIZE as usize);
            core::ptr::write_bytes(tx_ring as *mut u8, 0, pmm::PAGE_SIZE as usize);
        }

        let mut nic = Self {
            mmio,
            mac: [0; 6],
            rx_ring,
            tx_ring,
            rx_buffers,
            tx_buffers,
            rx_next: 0,
            tx_next: 0,
        };
        unsafe {
            nic.write(REG_IMC, u32::MAX);
            let ctrl = nic.read(REG_CTRL);
            nic.write(REG_CTRL, ctrl | (1 << 26));
        }
        let mut reset = false;
        for _ in 0..1_000_000 {
            if unsafe { nic.read(REG_CTRL) } & (1 << 26) == 0 {
                reset = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !reset {
            return Err("E1000 reset timed out");
        }
        unsafe { nic.write(REG_IMC, u32::MAX) };
        let ral = unsafe { nic.read(REG_RAL) };
        let rah = unsafe { nic.read(REG_RAH) };
        nic.mac = [ral as u8, (ral >> 8) as u8, (ral >> 16) as u8, (ral >> 24) as u8,
            rah as u8, (rah >> 8) as u8];
        if nic.mac.iter().all(|byte| *byte == 0) || nic.mac.iter().all(|byte| *byte == 0xff) {
            nic.mac = nic.read_mac_eeprom()?;
        }

        unsafe {
            for index in 0..RX_COUNT {
                let descriptor = (rx_ring as *mut RxDescriptor).add(index);
                (*descriptor).address = rx_buffers + (index * BUFFER_SIZE) as u64;
                (*descriptor).status = 0;
            }
            nic.write(REG_RDBAL, rx_ring as u32);
            nic.write(REG_RDBAH, (rx_ring >> 32) as u32);
            nic.write(REG_RDLEN, RING_BYTES as u32);
            nic.write(REG_RDH, 0);
            nic.write(REG_RDT, (RX_COUNT - 1) as u32);
            nic.write(REG_RCTL, (1 << 1) | (1 << 15) | (1 << 26)); // EN | BAM | SECRC, 2 KiB buffers

            nic.write(REG_TCTL, 0);
            for index in 0..TX_COUNT {
                let descriptor = (tx_ring as *mut TxDescriptor).add(index);
                (*descriptor).status = 1; // DD: initially available
            }
            nic.write(REG_TDBAL, tx_ring as u32);
            nic.write(REG_TDBAH, (tx_ring >> 32) as u32);
            nic.write(REG_TDLEN, (TX_COUNT * 16) as u32);
            nic.write(REG_TDH, 0);
            nic.write(REG_TDT, 0);
            nic.write(REG_TIPG, 0x0060_200a);
            nic.write(REG_TCTL, (1 << 1) | (1 << 3) | (0x10 << 4) | (0x40 << 12));
            nic.write(REG_CTRL, nic.read(REG_CTRL) | (1 << 6)); // Set link up.
        }
        Ok(nic)
    }

    fn read_mac_eeprom(&self) -> Result<[u8; 6], &'static str> {
        let mut words = [0u16; 3];
        for (index, word) in words.iter_mut().enumerate() {
            unsafe { self.write(REG_EERD, ((index as u32) << 8) | 1) };
            let mut done = false;
            for _ in 0..100_000 {
                let value = unsafe { self.read(REG_EERD) };
                if value & (1 << 4) != 0 {
                    *word = (value >> 16) as u16;
                    done = true;
                    break;
                }
                core::hint::spin_loop();
            }
            if !done {
                return Err("E1000 EEPROM read timed out");
            }
        }
        Ok([
            words[0] as u8, (words[0] >> 8) as u8,
            words[1] as u8, (words[1] >> 8) as u8,
            words[2] as u8, (words[2] >> 8) as u8,
        ])
    }

    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError> {
        if frame.is_empty() || frame.len() > BUFFER_SIZE {
            return Err(NetworkError::InvalidFrame);
        }
        let index = self.tx_next;
        let descriptor = unsafe { &mut *((self.tx_ring as *mut TxDescriptor).add(index)) };
        if descriptor.status & 1 == 0 {
            return Err(NetworkError::Busy);
        }
        let buffer = (self.tx_buffers as *mut u8).wrapping_add(index * BUFFER_SIZE);
        unsafe {
            core::ptr::copy_nonoverlapping(frame.as_ptr(), buffer, frame.len());
            descriptor.address = self.tx_buffers + (index * BUFFER_SIZE) as u64;
            descriptor.length = frame.len() as u16;
            descriptor.checksum_offset = 0;
            descriptor.command = (1 << 0) | (1 << 1) | (1 << 3); // EOP | IFCS | RS
            descriptor.status = 0;
            descriptor.checksum_start = 0;
            descriptor.special = 0;
            core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
            self.write(REG_TDT, ((index + 1) % TX_COUNT) as u32);
        }
        let mut completed = false;
        for _ in 0..100_000 {
            if descriptor.status & 1 != 0 {
                completed = true;
                break;
            }
            core::hint::spin_loop();
        }
        if !completed {
            return Err(NetworkError::Busy);
        }
        self.tx_next = (index + 1) % TX_COUNT;
        Ok(())
    }

    fn receive(&mut self, output: &mut [u8]) -> Result<Option<usize>, NetworkError> {
        let index = self.rx_next;
        let descriptor = unsafe { &mut *((self.rx_ring as *mut RxDescriptor).add(index)) };
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);
        if descriptor.status & 1 == 0 {
            return Ok(None);
        }
        let length = descriptor.length as usize;
        if length > output.len() || length > BUFFER_SIZE {
            descriptor.status = 0;
            unsafe { self.write(REG_RDT, index as u32) };
            self.rx_next = (index + 1) % RX_COUNT;
            return Err(NetworkError::InvalidFrame);
        }
        let buffer = (self.rx_buffers as *const u8).wrapping_add(index * BUFFER_SIZE);
        unsafe { core::ptr::copy_nonoverlapping(buffer, output.as_mut_ptr(), length) };
        descriptor.status = 0;
        unsafe { self.write(REG_RDT, index as u32) };
        self.rx_next = (index + 1) % RX_COUNT;
        Ok(Some(length))
    }
}

impl NetworkDevice for E1000 {
    fn mac_address(&self) -> [u8; 6] { self.mac }
    fn transmit(&mut self, frame: &[u8]) -> Result<(), NetworkError> { E1000::transmit(self, frame) }
    fn receive(&mut self, frame: &mut [u8]) -> Result<Option<usize>, NetworkError> {
        E1000::receive(self, frame)
    }
}

pub fn init() -> Result<[u8; 6], &'static str> {
    let nic = E1000::initialize()?;
    let mac = nic.mac;
    *DEVICE.lock() = Some(nic);
    Ok(mac)
}

pub fn mac_address() -> Option<[u8; 6]> { DEVICE.lock().as_ref().map(|nic| nic.mac) }
pub fn transmit(frame: &[u8]) -> Result<(), NetworkError> {
    DEVICE.lock().as_mut().ok_or(NetworkError::NotReady)?.transmit(frame)
}
pub fn receive(frame: &mut [u8]) -> Result<Option<usize>, NetworkError> {
    DEVICE.lock().as_mut().ok_or(NetworkError::NotReady)?.receive(frame)
}
