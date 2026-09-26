use crate::arch::x86_64::port::{in16, in8, out8, out16, wait};
use crate::device::{self, BlockDevice, BlockError};
use crate::sync::SpinLock;
use alloc::sync::Arc;

const DATA: u16 = 0x1f0;
const SECTOR_COUNT: u16 = 0x1f2;
const LBA_LOW: u16 = 0x1f3;
const LBA_MID: u16 = 0x1f4;
const LBA_HIGH: u16 = 0x1f5;
const DRIVE: u16 = 0x1f6;
const STATUS_COMMAND: u16 = 0x1f7;
const CONTROL_ALT_STATUS: u16 = 0x3f6;
const STATUS_ERR: u8 = 1;
const STATUS_DRQ: u8 = 8;
const STATUS_DF: u8 = 0x20;
const STATUS_BSY: u8 = 0x80;
const SECTOR_SIZE: usize = 512;
const POLL_LIMIT: usize = 2_000_000;

pub struct AtaPio {
    sectors: u64,
    drive_head: u8,
    lock: SpinLock<()>,
}

impl AtaPio {
    fn probe_drive(drive_head: u8) -> Option<Self> {
        unsafe {
            out8(DRIVE, drive_head);
            wait();
            out8(SECTOR_COUNT, 0);
            out8(LBA_LOW, 0);
            out8(LBA_MID, 0);
            out8(LBA_HIGH, 0);
            out8(STATUS_COMMAND, 0xec);
            if in8(STATUS_COMMAND) == 0 {
                return None;
            }
            if !Self::wait_not_busy() || in8(LBA_MID) != 0 || in8(LBA_HIGH) != 0 {
                return None;
            }
            if !Self::wait_data_ready() {
                return None;
            }
            let mut identify = [0u16; 256];
            for word in &mut identify {
                *word = in16(DATA);
            }
            let sectors = identify[60] as u64 | ((identify[61] as u64) << 16);
            if sectors == 0 {
                None
            } else {
                Some(Self {
                    sectors: sectors.min(1 << 28),
                    drive_head,
                    lock: SpinLock::new(()),
                })
            }
        }
    }

    unsafe fn wait_not_busy() -> bool {
        for _ in 0..POLL_LIMIT {
            let status = in8(CONTROL_ALT_STATUS);
            if status == 0xff {
                return false;
            }
            if status & STATUS_BSY == 0 {
                return status & (STATUS_ERR | STATUS_DF) == 0;
            }
            core::hint::spin_loop();
        }
        false
    }

    unsafe fn wait_data_ready() -> bool {
        for _ in 0..POLL_LIMIT {
            let status = in8(CONTROL_ALT_STATUS);
            if status == 0xff || status & (STATUS_ERR | STATUS_DF) != 0 {
                return false;
            }
            if status & STATUS_BSY == 0 && status & STATUS_DRQ != 0 {
                return true;
            }
            core::hint::spin_loop();
        }
        false
    }

    unsafe fn select_sector(&self, lba: u64, command: u8) -> bool {
        if !Self::wait_not_busy() || lba >= self.sectors || lba >= (1 << 28) {
            return false;
        }
        out8(DRIVE, self.drive_head | 0x40 | ((lba >> 24) as u8 & 0x0f));
        wait();
        out8(SECTOR_COUNT, 1);
        out8(LBA_LOW, lba as u8);
        out8(LBA_MID, (lba >> 8) as u8);
        out8(LBA_HIGH, (lba >> 16) as u8);
        out8(STATUS_COMMAND, command);
        true
    }

    unsafe fn read_sector(&self, lba: u64, output: &mut [u8]) -> bool {
        if !self.select_sector(lba, 0x20) || !Self::wait_data_ready() {
            return false;
        }
        for chunk in output.chunks_exact_mut(2) {
            chunk.copy_from_slice(&in16(DATA).to_le_bytes());
        }
        Self::wait_not_busy()
    }

    unsafe fn write_sector(&self, lba: u64, input: &[u8]) -> bool {
        if !self.select_sector(lba, 0x30) || !Self::wait_data_ready() {
            return false;
        }
        for chunk in input.chunks_exact(2) {
            out16(DATA, u16::from_le_bytes([chunk[0], chunk[1]]));
        }
        Self::wait_not_busy()
    }
}

impl BlockDevice for AtaPio {
    fn block_size(&self) -> u32 {
        SECTOR_SIZE as u32
    }

    fn block_count(&self) -> u64 {
        self.sectors
    }

    fn read_blocks(&self, lba: u64, output: &mut [u8]) -> Result<(), BlockError> {
        if output.len() % SECTOR_SIZE != 0 {
            return Err(BlockError::InvalidBuffer);
        }
        let count = (output.len() / SECTOR_SIZE) as u64;
        if lba.checked_add(count).filter(|end| *end <= self.sectors).is_none() {
            return Err(BlockError::OutOfRange);
        }
        let _guard = self.lock.lock();
        for (index, sector) in output.chunks_exact_mut(SECTOR_SIZE).enumerate() {
            if !unsafe { self.read_sector(lba + index as u64, sector) } {
                return Err(BlockError::Io);
            }
        }
        Ok(())
    }

    fn write_blocks(&self, lba: u64, input: &[u8]) -> Result<(), BlockError> {
        if input.len() % SECTOR_SIZE != 0 {
            return Err(BlockError::InvalidBuffer);
        }
        let count = (input.len() / SECTOR_SIZE) as u64;
        if lba.checked_add(count).filter(|end| *end <= self.sectors).is_none() {
            return Err(BlockError::OutOfRange);
        }
        let _guard = self.lock.lock();
        for (index, sector) in input.chunks_exact(SECTOR_SIZE).enumerate() {
            if !unsafe { self.write_sector(lba + index as u64, sector) } {
                return Err(BlockError::Io);
            }
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), BlockError> {
        let _guard = self.lock.lock();
        unsafe {
            if !Self::wait_not_busy() {
                return Err(BlockError::Io);
            }
            out8(DRIVE, self.drive_head | 0x40);
            out8(STATUS_COMMAND, 0xe7);
            if !Self::wait_not_busy() {
                return Err(BlockError::Io);
            }
        }
        Ok(())
    }
}

pub fn init() {
    // In the QEMU layout the UEFI FAT volume is primary master and SBFS is
    // primary slave. Fall back to the master for machines with a single disk.
    if let Some(disk) = AtaPio::probe_drive(0xb0).or_else(|| AtaPio::probe_drive(0xa0)) {
        let sectors = disk.sectors;
        let disk: Arc<dyn BlockDevice> = Arc::new(disk);
        device::register_block_device("QEMU", "Primary IDE disk", "ata-pio", disk);
        crate::kprintln!("ATA PIO: primary master online, {} sectors", sectors);
    } else {
        crate::kprintln!("ATA PIO: no primary master disk detected");
    }
}
