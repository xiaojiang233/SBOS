use crate::bootinfo::MemoryDescriptor;
use crate::sync::SpinLock;

pub const PAGE_SIZE: u64 = 4096;
// Phase one keeps a 1 GiB direct map; higher device MMIO is mapped separately.
pub const MAX_PHYSICAL_ADDRESS: u64 = 0x4000_0000;
const FRAME_COUNT: usize = (MAX_PHYSICAL_ADDRESS / PAGE_SIZE) as usize;
const BITMAP_WORDS: usize = FRAME_COUNT / 64;

struct FrameBitmap {
    words: [u64; BITMAP_WORDS],
    free_frames: usize,
}
static FRAMES: SpinLock<FrameBitmap> = SpinLock::new(FrameBitmap {
    words: [u64::MAX; BITMAP_WORDS],
    free_frames: 0,
});

pub fn init(map: u64, map_size: usize, descriptor_size: usize) -> Result<(), &'static str> {
    if map == 0
        || descriptor_size < core::mem::size_of::<MemoryDescriptor>()
        || map_size < descriptor_size
    {
        return Err("invalid UEFI memory map");
    }
    let mut frames = FRAMES.lock();
    frames.words.fill(u64::MAX);
    frames.free_frames = 0;
    let count = map_size / descriptor_size;
    for i in 0..count {
        let descriptor = unsafe {
            core::ptr::read_unaligned(
                (map as *const u8)
                    .add(i * descriptor_size)
                    .cast::<MemoryDescriptor>(),
            )
        };
        // UEFI ConventionalMemory is the only class returned to the allocator.
        // Loader, runtime, ACPI, MMIO, kernel, shell and boot-info pages stay reserved.
        if descriptor.memory_type != 7 {
            continue;
        }
        let begin = descriptor.physical_start.max(0x10_0000) / PAGE_SIZE;
        let end = descriptor
            .physical_start
            .saturating_add(descriptor.number_of_pages.saturating_mul(PAGE_SIZE))
            .min(MAX_PHYSICAL_ADDRESS)
            / PAGE_SIZE;
        for frame in begin..end {
            let index = frame as usize;
            if index >= FRAME_COUNT {
                break;
            }
            let mask = 1u64 << (index & 63);
            if frames.words[index >> 6] & mask != 0 {
                frames.words[index >> 6] &= !mask;
                frames.free_frames += 1;
            }
        }
    }
    if frames.free_frames == 0 {
        return Err("no conventional memory below the 1 GiB direct-map limit");
    }
    Ok(())
}

pub fn allocate_frame() -> Option<u64> {
    let mut frames = FRAMES.lock();
    if frames.free_frames == 0 {
        return None;
    }
    for (word_index, word) in frames.words.iter_mut().enumerate() {
        let free_bits = !*word;
        if free_bits == 0 {
            continue;
        }
        let bit = free_bits.trailing_zeros() as usize;
        *word |= 1u64 << bit;
        frames.free_frames -= 1;
        return Some(((word_index * 64 + bit) as u64) * PAGE_SIZE);
    }
    None
}

pub fn allocate_contiguous_pages(count: usize) -> Option<u64> {
    if count == 0 {
        return None;
    }
    let mut frames = FRAMES.lock();
    let mut run = 0usize;
    let mut start = 0usize;
    for index in 0..FRAME_COUNT {
        let used = frames.words[index >> 6] & (1u64 << (index & 63)) != 0;
        if used {
            run = 0;
            continue;
        }
        if run == 0 {
            start = index;
        }
        run += 1;
        if run == count {
            for frame in start..start + count {
                frames.words[frame >> 6] |= 1u64 << (frame & 63);
            }
            frames.free_frames = frames.free_frames.saturating_sub(count);
            return Some(start as u64 * PAGE_SIZE);
        }
    }
    None
}

pub fn free_frame(address: u64) -> Result<(), &'static str> {
    if address % PAGE_SIZE != 0 || address >= MAX_PHYSICAL_ADDRESS {
        return Err("invalid physical frame");
    }
    let index = (address / PAGE_SIZE) as usize;
    let mut frames = FRAMES.lock();
    let mask = 1u64 << (index & 63);
    if frames.words[index >> 6] & mask != 0 {
        frames.words[index >> 6] &= !mask;
        frames.free_frames += 1;
        Ok(())
    } else {
        Err("double free or reserved frame")
    }
}

pub fn free_frame_count() -> usize {
    FRAMES.lock().free_frames
}
