use crate::memory::pmm::{self, PAGE_SIZE};
use crate::sync::SpinLock;
use core::arch::asm;

const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const USER: u64 = 1 << 2;
const WRITE_THROUGH: u64 = 1 << 3;
const CACHE_DISABLE: u64 = 1 << 4;
const HUGE: u64 = 1 << 7;
const NO_EXECUTE: u64 = 1 << 63;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
pub const USER_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;
const HIGH_FB_BASE: u64 = 0xffff_9000_0000_0000;
const HIGH_MMIO_BASE: u64 = 0xffff_a000_0000_0000;

#[derive(Clone, Copy)]
pub struct PageFlags {
    pub user: bool,
    pub writable: bool,
    pub executable: bool,
    pub cache_disable: bool,
}
impl PageFlags {
    pub const KERNEL: Self = Self {
        user: false,
        writable: true,
        executable: true,
        cache_disable: false,
    };
    pub const KERNEL_CODE: Self = Self {
        user: false,
        writable: false,
        executable: true,
        cache_disable: false,
    };
    pub const KERNEL_DATA: Self = Self {
        user: false,
        writable: true,
        executable: false,
        cache_disable: false,
    };
    pub const KERNEL_RODATA: Self = Self {
        user: false,
        writable: false,
        executable: false,
        cache_disable: false,
    };
    pub const USER_CODE: Self = Self {
        user: true,
        writable: false,
        executable: true,
        cache_disable: false,
    };
    pub const USER_DATA: Self = Self {
        user: true,
        writable: true,
        executable: false,
        cache_disable: false,
    };
}

static ROOT: SpinLock<u64> = SpinLock::new(0);
static KERNEL_ROOT: SpinLock<u64> = SpinLock::new(0);

unsafe fn allocate_table() -> Result<*mut u64, &'static str> {
    let physical = pmm::allocate_frame().ok_or("out of physical memory for page tables")?;
    let table = physical as *mut u64;
    core::ptr::write_bytes(table, 0, 512);
    Ok(table)
}

unsafe fn split_huge(entry: &mut u64) -> Result<(), &'static str> {
    let old = *entry;
    let table = allocate_table()?;
    let base = old & 0x000f_ffff_ffe0_0000;
    let inherited = (old
        & (WRITABLE | USER | WRITE_THROUGH | CACHE_DISABLE | (1 << 3) | (1 << 4) | NO_EXECUTE))
        & !HUGE;
    for index in 0..512 {
        table
            .add(index)
            .write(base + index as u64 * PAGE_SIZE | (old & 0xfff & !HUGE) | inherited | PRESENT);
    }
    *entry = table as u64 | PRESENT | WRITABLE | (old & USER);
    Ok(())
}

unsafe fn ensure_table(
    parent: *mut u64,
    index: usize,
    user: bool,
) -> Result<*mut u64, &'static str> {
    let entry = &mut *parent.add(index);
    if *entry & PRESENT == 0 {
        let table = allocate_table()?;
        *entry = table as u64 | PRESENT | WRITABLE | if user { USER } else { 0 };
        return Ok(table);
    }
    if *entry & HUGE != 0 {
        split_huge(entry)?;
    }
    if user {
        *entry |= USER;
    }
    Ok((*entry & ADDRESS_MASK) as *mut u64)
}

pub unsafe fn init(framebuffer_base: u64, framebuffer_size: u64) -> Result<(), &'static str> {
    let root = allocate_table()?;
    let pdpt = allocate_table()?;
    root.add(0).write(pdpt as u64 | PRESENT | WRITABLE);
    for pdpt_index in 0..1 {
        let pd = allocate_table()?;
        pdpt.add(pdpt_index).write(pd as u64 | PRESENT | WRITABLE);
        for index in 0..512 {
            let physical = (pdpt_index as u64 * 512 + index as u64) * 0x20_0000;
            pd.add(index)
                .write(physical | PRESENT | WRITABLE | HUGE | NO_EXECUTE);
        }
    }
    *ROOT.lock() = root as u64;
    *KERNEL_ROOT.lock() = root as u64;
    if framebuffer_base >= pmm::MAX_PHYSICAL_ADDRESS && framebuffer_size != 0 {
        let page_start = framebuffer_base & !(PAGE_SIZE - 1);
        let page_end = framebuffer_base
            .saturating_add(framebuffer_size)
            .saturating_add(PAGE_SIZE - 1)
            & !(PAGE_SIZE - 1);
        for offset in (0..page_end.saturating_sub(page_start)).step_by(PAGE_SIZE as usize) {
            map_page_in(
                root as u64,
                HIGH_FB_BASE + offset,
                page_start + offset,
                PageFlags {
                    user: false,
                    writable: true,
                    executable: false,
                    cache_disable: true,
                },
            )?;
        }
    }
    extern "C" {
        static _text_start: u8;
        static _text_end: u8;
        static _rodata_start: u8;
        static _rodata_end: u8;
        static _data_start: u8;
        static _data_end: u8;
    }
    protect_kernel_range(
        root as u64,
        core::ptr::addr_of!(_text_start) as u64,
        core::ptr::addr_of!(_text_end) as u64,
        PageFlags::KERNEL_CODE,
    )?;
    protect_kernel_range(
        root as u64,
        core::ptr::addr_of!(_rodata_start) as u64,
        core::ptr::addr_of!(_rodata_end) as u64,
        PageFlags::KERNEL_RODATA,
    )?;
    protect_kernel_range(
        root as u64,
        core::ptr::addr_of!(_data_start) as u64,
        core::ptr::addr_of!(_data_end) as u64,
        PageFlags::KERNEL_DATA,
    )?;
    let mut efer_low: u32;
    let mut efer_high: u32;
    asm!("rdmsr", in("ecx") 0xc000_0080u32, out("eax") efer_low, out("edx") efer_high, options(nomem, nostack));
    efer_low |= 1 << 11; // NXE
    asm!("wrmsr", in("ecx") 0xc000_0080u32, in("eax") efer_low, in("edx") efer_high, options(nomem, nostack));
    let cr0: u64;
    asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    asm!("mov cr0, {}", in(reg) cr0 | (1 << 16), options(nomem, nostack, preserves_flags)); // supervisor write protection
    asm!("mov cr3, {}", in(reg) root as u64, options(nostack, preserves_flags));
    Ok(())
}

unsafe fn protect_kernel_range(
    root: u64,
    start: u64,
    end: u64,
    flags: PageFlags,
) -> Result<(), &'static str> {
    let begin = start & !(PAGE_SIZE - 1);
    let limit = (end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
    for address in (begin..limit).step_by(PAGE_SIZE as usize) {
        map_page_in(root, address, address, flags)?;
    }
    Ok(())
}

unsafe fn map_page_in(
    root: u64,
    virtual_address: u64,
    physical_address: u64,
    flags: PageFlags,
) -> Result<(), &'static str> {
    if virtual_address % PAGE_SIZE != 0 || physical_address % PAGE_SIZE != 0 {
        return Err("page mapping is not aligned");
    }
    let pml4 = root as *mut u64;
    let user = flags.user;
    let pdpt = ensure_table(pml4, ((virtual_address >> 39) & 0x1ff) as usize, user)?;
    let pd = ensure_table(pdpt, ((virtual_address >> 30) & 0x1ff) as usize, user)?;
    let pt = ensure_table(pd, ((virtual_address >> 21) & 0x1ff) as usize, user)?;
    let pte = pt.add(((virtual_address >> 12) & 0x1ff) as usize);
    let mut entry = physical_address | PRESENT;
    if flags.writable {
        entry |= WRITABLE;
    }
    if flags.user {
        entry |= USER;
    }
    if !flags.executable {
        entry |= NO_EXECUTE;
    }
    if flags.cache_disable {
        entry |= CACHE_DISABLE | WRITE_THROUGH;
    }
    pte.write(entry);
    asm!("invlpg [{}]", in(reg) virtual_address, options(nostack, preserves_flags));
    Ok(())
}

pub fn map_page(
    virtual_address: u64,
    physical_address: u64,
    flags: PageFlags,
) -> Result<(), &'static str> {
    let root = *ROOT.lock();
    if root == 0 {
        return Err("VMM is not initialized");
    }
    unsafe { map_page_in(root, virtual_address, physical_address, flags) }
}

/// Map a device MMIO range into a reserved kernel virtual window.
/// The returned address has the same page offset as `physical_address`.
pub fn map_mmio(physical_address: u64, size: u64, slot: u64) -> Result<u64, &'static str> {
    if size == 0 || slot >= 256 * 1024 * 1024 {
        return Err("invalid MMIO range");
    }
    let page_start = physical_address & !(PAGE_SIZE - 1);
    let offset = physical_address - page_start;
    let span = size.checked_add(offset).ok_or("MMIO range overflow")?;
    let page_count = span
        .checked_add(PAGE_SIZE - 1)
        .ok_or("MMIO range overflow")?
        / PAGE_SIZE;
    let virtual_start = HIGH_MMIO_BASE
        .checked_add(slot.checked_mul(256 * 1024 * 1024).ok_or("MMIO slot overflow")?)
        .ok_or("MMIO virtual address overflow")?;
    for page in 0..page_count {
        map_page(
            virtual_start + page * PAGE_SIZE,
            page_start + page * PAGE_SIZE,
            PageFlags {
                user: false,
                writable: true,
                executable: false,
                cache_disable: true,
            },
        )?;
    }
    Ok(virtual_start + offset)
}

pub fn map_page_in_root(
    root: u64,
    virtual_address: u64,
    physical_address: u64,
    flags: PageFlags,
) -> Result<(), &'static str> {
    if root == 0 {
        return Err("invalid address space");
    }
    unsafe { map_page_in(root, virtual_address, physical_address, flags) }
}

unsafe fn release_clone_table(table: u64, level: u8, copied_user_pages: bool) {
    let entries = table as *mut u64;
    for index in 0..512 {
        let entry = entries.add(index).read();
        if entry & PRESENT == 0 {
            continue;
        }
        if level == 1 {
            if copied_user_pages && entry & USER != 0 {
                let _ = pmm::free_frame(entry & ADDRESS_MASK);
            }
        } else if level == 2 && entry & HUGE != 0 {
            // Huge supervisor mappings refer to shared kernel memory.
        } else {
            let child = entry & ADDRESS_MASK;
            release_clone_table(child, level - 1, copied_user_pages);
            let _ = pmm::free_frame(child);
        }
    }
    let _ = pmm::free_frame(table);
}

unsafe fn clone_table(
    source: u64,
    level: u8,
    copy_user_pages: bool,
) -> Result<u64, &'static str> {
    let destination = allocate_table()? as u64;
    let old = source as *const u64;
    for index in 0..512 {
        let entry = old.add(index).read();
        if entry & PRESENT == 0 {
            continue;
        }
        if level == 1 || (level == 2 && entry & HUGE != 0) {
            if entry & USER == 0 {
                (destination as *mut u64).add(index).write(entry);
            } else if copy_user_pages {
                if level != 1 {
                    release_clone_table(destination, level, copy_user_pages);
                    return Err("fork does not support huge user mappings");
                }
                let Some(frame) = pmm::allocate_frame() else {
                    release_clone_table(destination, level, copy_user_pages);
                    return Err("out of memory copying a user page");
                };
                core::ptr::copy_nonoverlapping(
                    (entry & ADDRESS_MASK) as *const u8,
                    frame as *mut u8,
                    PAGE_SIZE as usize,
                );
                (destination as *mut u64)
                    .add(index)
                    .write((entry & !ADDRESS_MASK) | frame);
            }
        } else {
            let child = match clone_table(entry & ADDRESS_MASK, level - 1, copy_user_pages) {
                Ok(child) => child,
                Err(error) => {
                    release_clone_table(destination, level, copy_user_pages);
                    return Err(error);
                }
            };
            (destination as *mut u64)
                .add(index)
                .write((entry & !ADDRESS_MASK) | child);
        }
    }
    Ok(destination)
}

pub fn clone_kernel_root() -> Result<u64, &'static str> {
    let source = *KERNEL_ROOT.lock();
    if source == 0 {
        return Err("kernel address space is not initialized");
    }
    unsafe { clone_table(source, 4, false) }
}

pub fn clone_address_space(source: u64) -> Result<u64, &'static str> {
    if source == 0 {
        return Err("invalid source address space");
    }
    unsafe { clone_table(source, 4, true) }
}

pub fn destroy_address_space(root: u64) -> Result<(), &'static str> {
    if root == 0 || root == *KERNEL_ROOT.lock() {
        return Err("cannot destroy this address space");
    }
    if root == *ROOT.lock() {
        return Err("cannot destroy the active address space");
    }
    unsafe { release_clone_table(root, 4, true) };
    Ok(())
}

pub fn switch_root(root: u64) -> Result<(), &'static str> {
    if root == 0 {
        return Err("invalid address space root");
    }
    *ROOT.lock() = root;
    unsafe {
        asm!("mov cr3, {}",in(reg)root,options(nostack,preserves_flags));
    }
    Ok(())
}

pub fn unmap_page(virtual_address: u64) -> Result<u64, &'static str> {
    if virtual_address % PAGE_SIZE != 0 {
        return Err("page mapping is not aligned");
    }
    let root = *ROOT.lock();
    if root == 0 {
        return Err("VMM is not initialized");
    }
    unsafe {
        let pml4 = root as *mut u64;
        let a = *pml4.add(((virtual_address >> 39) & 0x1ff) as usize);
        if a & PRESENT == 0 {
            return Err("mapping not found");
        }
        let pdpt = (a & ADDRESS_MASK) as *mut u64;
        let b = *pdpt.add(((virtual_address >> 30) & 0x1ff) as usize);
        if b & PRESENT == 0 {
            return Err("mapping not found");
        }
        let pd = (b & ADDRESS_MASK) as *mut u64;
        let c = *pd.add(((virtual_address >> 21) & 0x1ff) as usize);
        if c & PRESENT == 0 || c & HUGE != 0 {
            return Err("mapping not found");
        }
        let pt = (c & ADDRESS_MASK) as *mut u64;
        let pte = pt.add(((virtual_address >> 12) & 0x1ff) as usize);
        let old = pte.read();
        if old & PRESENT == 0 || old & USER == 0 {
            return Err("user mapping not found");
        }
        pte.write(0);
        asm!("invlpg [{}]", in(reg) virtual_address, options(nostack, preserves_flags));
        Ok(old & ADDRESS_MASK)
    }
}

pub fn unmap_page_in_root(root: u64, virtual_address: u64) -> Result<u64, &'static str> {
    let old = *ROOT.lock();
    switch_root(root)?;
    let result = unmap_page(virtual_address);
    let _ = switch_root(old);
    result
}

pub fn framebuffer_address(physical: u64) -> u64 {
    if physical < pmm::MAX_PHYSICAL_ADDRESS {
        physical
    } else {
        HIGH_FB_BASE + (physical & (PAGE_SIZE - 1))
    }
}

pub fn is_user_range(address: u64, length: usize, write: bool) -> bool {
    if length == 0 {
        return true;
    }
    let Some(end) = address.checked_add(length as u64 - 1) else {
        return false;
    };
    if address < 0x10000 || end >= 0x0000_8000_0000_0000 {
        return false;
    }
    let root = *ROOT.lock();
    if root == 0 {
        return false;
    }
    let mut page = address & !(PAGE_SIZE - 1);
    let last = end & !(PAGE_SIZE - 1);
    loop {
        let flags = unsafe { leaf_entry(root, page) };
        if flags & (PRESENT | USER) != PRESENT | USER || (write && flags & WRITABLE == 0) {
            return false;
        }
        if page == last {
            return true;
        }
        page = page.saturating_add(PAGE_SIZE);
    }
}

pub fn is_user_executable(address: u64) -> bool {
    if address < 0x10000 || address >= 0x0000_8000_0000_0000 {
        return false;
    }
    let root = *ROOT.lock();
    if root == 0 {
        return false;
    }
    let entry = unsafe { leaf_entry(root, address) };
    entry & (PRESENT | USER) == PRESENT | USER && entry & NO_EXECUTE == 0
}

unsafe fn leaf_entry(root: u64, address: u64) -> u64 {
    let pml4 = root as *mut u64;
    let a = pml4.add(((address >> 39) & 0x1ff) as usize).read();
    if a & PRESENT == 0 {
        return 0;
    }
    let pdpt = (a & ADDRESS_MASK) as *mut u64;
    let b = pdpt.add(((address >> 30) & 0x1ff) as usize).read();
    if b & PRESENT == 0 {
        return 0;
    }
    if b & HUGE != 0 {
        return b;
    }
    let pd = (b & ADDRESS_MASK) as *mut u64;
    let c = pd.add(((address >> 21) & 0x1ff) as usize).read();
    if c & PRESENT == 0 {
        return 0;
    }
    if c & HUGE != 0 {
        return c;
    }
    let pt = (c & ADDRESS_MASK) as *mut u64;
    pt.add(((address >> 12) & 0x1ff) as usize).read()
}

pub fn copy_from_user(destination: &mut [u8], source: u64) -> Result<(), &'static str> {
    if !is_user_range(source, destination.len(), false) {
        return Err("invalid user read range");
    }
    unsafe {
        core::ptr::copy_nonoverlapping(
            source as *const u8,
            destination.as_mut_ptr(),
            destination.len(),
        );
    }
    Ok(())
}

pub fn copy_to_user(destination: u64, source: &[u8]) -> Result<(), &'static str> {
    if !is_user_range(destination, source.len(), true) {
        return Err("invalid user write range");
    }
    unsafe {
        core::ptr::copy_nonoverlapping(source.as_ptr(), destination as *mut u8, source.len());
    }
    Ok(())
}

pub fn current_root() -> u64 {
    *ROOT.lock()
}

pub fn kernel_root() -> u64 {
    *KERNEL_ROOT.lock()
}
