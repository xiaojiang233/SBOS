use crate::memory::{pmm, vmm};
use crate::task::process::AddressSpace;
use core::ptr::{copy_nonoverlapping, read_unaligned, write_bytes};

#[repr(C)]
#[derive(Clone, Copy)]
struct ElfHeader {
    ident: [u8; 16],
    kind: u16,
    machine: u16,
    version: u32,
    entry: u64,
    phoff: u64,
    shoff: u64,
    flags: u32,
    ehsize: u16,
    phentsize: u16,
    phnum: u16,
    shentsize: u16,
    shnum: u16,
    shstrndx: u16,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct ProgramHeader {
    kind: u32,
    flags: u32,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    filesz: u64,
    memsz: u64,
    align: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct LoadedImage {
    pub entry: u64,
    pub stack_pointer: u64,
}

pub fn load_user(
    image: &[u8],
    address_space: &AddressSpace,
    argv: &[&str],
    envp: &[&str],
) -> Result<LoadedImage, &'static str> {
    if image.len() < core::mem::size_of::<ElfHeader>() {
        return Err("truncated ELF header");
    }
    let header = unsafe { read_unaligned(image.as_ptr().cast::<ElfHeader>()) };
    if &header.ident[..4] != b"\x7fELF"
        || header.ident[4] != 2
        || header.ident[5] != 1
        || header.machine != 0x3e
        || header.kind != 2
    {
        return Err("unsupported executable format");
    }
    if header.phentsize as usize != core::mem::size_of::<ProgramHeader>() {
        return Err("invalid ELF program-header size");
    }
    let table_end = header
        .phoff
        .checked_add(header.phnum as u64 * header.phentsize as u64)
        .ok_or("ELF header overflow")?;
    if table_end > image.len() as u64 {
        return Err("ELF program headers are truncated");
    }
    let mut entry_is_executable = false;
    for index in 0..header.phnum as usize {
        let ph = unsafe {
            read_unaligned(
                image
                    .as_ptr()
                    .add(header.phoff as usize + index * header.phentsize as usize)
                    .cast::<ProgramHeader>(),
            )
        };
        if ph.kind != 1 || ph.memsz == 0 {
            continue;
        }
        if ph.filesz > ph.memsz
            || ph
                .offset
                .checked_add(ph.filesz)
                .ok_or("ELF segment overflow")?
                > image.len() as u64
        {
            return Err("ELF segment exceeds file");
        }
        let segment_end = ph
            .vaddr
            .checked_add(ph.memsz)
            .ok_or("ELF virtual address overflow")?;
        if ph.vaddr < 0x10000 || segment_end >= 0x0000_8000_0000_0000 {
            return Err("ELF segment outside user address space");
        }
        let first = ph.vaddr & !(pmm::PAGE_SIZE - 1);
        let last = (segment_end + pmm::PAGE_SIZE - 1) & !(pmm::PAGE_SIZE - 1);
        let flags = vmm::PageFlags {
            user: true,
            writable: ph.flags & 2 != 0,
            executable: ph.flags & 1 != 0,
            cache_disable: false,
        };
        for page in (first..last).step_by(pmm::PAGE_SIZE as usize) {
            let physical = pmm::allocate_frame().ok_or("out of memory loading executable")?;
            unsafe {
                write_bytes(physical as *mut u8, 0, pmm::PAGE_SIZE as usize);
            }
            vmm::map_page_in_root(address_space.root(), page, physical, flags)?;
            let copy_start = ph.vaddr.max(page);
            let copy_end = segment_end.min(page + pmm::PAGE_SIZE);
            if copy_end > copy_start {
                let file_offset = ph.offset + (copy_start - ph.vaddr);
                let file_end = (copy_end - ph.vaddr).min(ph.filesz);
                if file_end > copy_start - ph.vaddr {
                    let amount = (file_end - (copy_start - ph.vaddr)) as usize;
                    unsafe {
                        copy_nonoverlapping(
                            image.as_ptr().add(file_offset as usize),
                            (physical + (copy_start - page)) as *mut u8,
                            amount,
                        );
                    }
                }
            }
            address_space.record(
                page,
                pmm::PAGE_SIZE,
                flags.user as u8 | (flags.writable as u8) << 1 | (flags.executable as u8) << 2,
            );
        }
        if flags.executable && header.entry >= ph.vaddr && header.entry < segment_end {
            entry_is_executable = true;
        }
    }
    if !entry_is_executable {
        return Err("ELF entry is not in an executable segment");
    }
    if argv.len() > 32 || envp.len() > 32 {
        return Err("too many process arguments or environment entries");
    }
    let string_bytes = argv
        .iter()
        .chain(envp.iter())
        .try_fold(0usize, |sum, string| sum.checked_add(string.len() + 1))
        .ok_or("process argument size overflow")?;
    if string_bytes > 2048 {
        return Err("process arguments exceed the initial stack budget");
    }

    // Eight pages provide a useful bootstrap stack while keeping the mapping small.
    let stack_start = vmm::USER_STACK_TOP - 8 * pmm::PAGE_SIZE;
    let mut top_frame = 0;
    for page in (stack_start..vmm::USER_STACK_TOP).step_by(pmm::PAGE_SIZE as usize) {
        let physical = pmm::allocate_frame().ok_or("out of memory allocating user stack")?;
        if page == vmm::USER_STACK_TOP - pmm::PAGE_SIZE {
            top_frame = physical;
        }
        unsafe {
            write_bytes(physical as *mut u8, 0, pmm::PAGE_SIZE as usize);
        }
        vmm::map_page_in_root(
            address_space.root(),
            page,
            physical,
            vmm::PageFlags::USER_DATA,
        )?;
        address_space.record(page, pmm::PAGE_SIZE, 3);
    }
    let stack_base = vmm::USER_STACK_TOP - pmm::PAGE_SIZE;
    let stack = unsafe {
        core::slice::from_raw_parts_mut(top_frame as *mut u8, pmm::PAGE_SIZE as usize)
    };
    let mut cursor = stack.len();
    let mut argv_pointers = alloc::vec::Vec::with_capacity(argv.len());
    let mut envp_pointers = alloc::vec::Vec::with_capacity(envp.len());
    for (strings, pointers) in [(argv, &mut argv_pointers), (envp, &mut envp_pointers)] {
        for value in strings {
            let length = value.len() + 1;
            cursor = cursor.checked_sub(length).ok_or("process strings exceed stack")?;
            stack[cursor..cursor + value.len()].copy_from_slice(value.as_bytes());
            stack[cursor + value.len()] = 0;
            pointers.push(stack_base + cursor as u64);
        }
    }
    let mut words = alloc::vec::Vec::with_capacity(1 + argv.len() + envp.len() + 9);
    words.push(argv.len() as u64);
    words.extend(argv_pointers.iter().copied());
    words.push(0);
    words.extend(envp_pointers.iter().copied());
    words.push(0);
    words.extend([6, pmm::PAGE_SIZE, 9, header.entry, 0, 0]); // AT_PAGESZ, AT_ENTRY, AT_NULL
    let table_bytes = words.len() * core::mem::size_of::<u64>();
    let table_start = cursor
        .checked_sub(table_bytes)
        .ok_or("process argument table exceeds stack")?
        & !15;
    for (index, word) in words.iter().enumerate() {
        let offset = table_start + index * 8;
        stack[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
    }
    let stack_pointer = stack_base + table_start as u64;
    Ok(LoadedImage {
        entry: header.entry,
        stack_pointer,
    })
}
