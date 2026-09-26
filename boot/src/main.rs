#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;
use core::ptr::{copy_nonoverlapping, read_unaligned, write_bytes};

const EFI_SUCCESS: usize = 0;
const EFI_ERROR_BIT: usize = 1usize << (usize::BITS - 1);
const EFI_LOADER_DATA: u32 = 2;
const ALLOCATE_ANY_PAGES: u32 = 0;
const ALLOCATE_MAX_ADDRESS: u32 = 1;
const ALLOCATE_ADDRESS: u32 = 2;
const OPEN_PROTOCOL_GET_PROTOCOL: u32 = 2;
const FILE_MODE_READ: u64 = 1;
const KERNEL_LIMIT: u64 = 0x4000_0000;
const BOOT_DATA_MAX: u64 = KERNEL_LIMIT - 1;
const BOOT_MAGIC: u64 = 0x5342_4f53_424f_4f54; // "SBOSBOOT"

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

const LOADED_IMAGE_GUID: Guid = Guid {
    a: 0x5b1b31a1,
    b: 0x9562,
    c: 0x11d2,
    d: [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const SIMPLE_FS_GUID: Guid = Guid {
    a: 0x964e5b22,
    b: 0x6459,
    c: 0x11d2,
    d: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};
const GOP_GUID: Guid = Guid {
    a: 0x9042a9de,
    b: 0x23dc,
    c: 0x4a38,
    d: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};
const FILE_INFO_GUID: Guid = Guid {
    a: 0x09576e92,
    b: 0x6d3f,
    c: 0x11d2,
    d: [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b],
};

type Status = usize;
type Handle = *mut core::ffi::c_void;

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct SystemTable {
    header: TableHeader,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    _padding: u32,
    console_in_handle: Handle,
    console_in: *mut core::ffi::c_void,
    console_out_handle: Handle,
    console_out: *mut core::ffi::c_void,
    standard_error_handle: Handle,
    standard_error: *mut core::ffi::c_void,
    runtime_services: *mut core::ffi::c_void,
    boot_services: *mut BootServices,
    configuration_table_entries: usize,
    configuration_table: *const ConfigurationTable,
}

#[repr(C)]
struct SimpleTextOutputProtocol {
    reset: usize,
    output_string: unsafe extern "efiapi" fn(*mut Self, *const u16) -> Status,
}

#[repr(C)]
struct ConfigurationTable {
    vendor_guid: Guid,
    vendor_table: *const u8,
}

#[repr(C)]
struct BootServices {
    header: TableHeader,
    raise_tpl: usize,
    restore_tpl: usize,
    allocate_pages: unsafe extern "efiapi" fn(u32, u32, usize, *mut u64) -> Status,
    free_pages: unsafe extern "efiapi" fn(u64, usize) -> Status,
    get_memory_map: unsafe extern "efiapi" fn(
        *mut usize,
        *mut MemoryDescriptor,
        *mut usize,
        *mut usize,
        *mut u32,
    ) -> Status,
    allocate_pool: unsafe extern "efiapi" fn(u32, usize, *mut *mut u8) -> Status,
    free_pool: unsafe extern "efiapi" fn(*mut u8) -> Status,
    _events_and_protocols: [usize; 9],
    handle_protocol:
        unsafe extern "efiapi" fn(Handle, *const Guid, *mut *mut core::ffi::c_void) -> Status,
    _reserved: usize,
    _register_protocol_notify: usize,
    _locate_handle: usize,
    _locate_device_path: usize,
    _install_configuration_table: usize,
    _load_image: usize,
    _start_image: usize,
    _exit: usize,
    _unload_image: usize,
    exit_boot_services: unsafe extern "efiapi" fn(Handle, usize) -> Status,
    _next_monotonic_count: usize,
    _stall: usize,
    _set_watchdog_timer: usize,
    _connect_controller: usize,
    _disconnect_controller: usize,
    open_protocol: unsafe extern "efiapi" fn(
        Handle,
        *const Guid,
        *mut *mut core::ffi::c_void,
        Handle,
        Handle,
        u32,
    ) -> Status,
    _close_protocol: usize,
    _open_protocol_information: usize,
    _protocols_per_handle: usize,
    _locate_handle_buffer: usize,
    locate_protocol: unsafe extern "efiapi" fn(
        *const Guid,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> Status,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MemoryDescriptor {
    memory_type: u32,
    _padding: u32,
    physical_start: u64,
    virtual_start: u64,
    number_of_pages: u64,
    attributes: u64,
}

#[repr(C)]
struct LoadedImageProtocol {
    revision: u32,
    parent_handle: Handle,
    system_table: *mut SystemTable,
    device_handle: Handle,
    file_path: *mut core::ffi::c_void,
    reserved: *mut core::ffi::c_void,
    load_options_size: u32,
    _padding: u32,
    load_options: *mut core::ffi::c_void,
    image_base: *mut u8,
    image_size: u64,
    image_code_type: u32,
    image_data_type: u32,
    unload: usize,
}

#[repr(C)]
struct FileSystemProtocol {
    revision: u64,
    open_volume: unsafe extern "efiapi" fn(*mut Self, *mut *mut FileProtocol) -> Status,
}

#[repr(C)]
struct FileProtocol {
    revision: u64,
    open: unsafe extern "efiapi" fn(
        *mut Self,
        *mut *mut FileProtocol,
        *const u16,
        u64,
        u64,
    ) -> Status,
    close: unsafe extern "efiapi" fn(*mut Self) -> Status,
    _delete: usize,
    read: unsafe extern "efiapi" fn(*mut Self, *mut usize, *mut u8) -> Status,
    _write_and_position_methods: [usize; 3],
    get_info: unsafe extern "efiapi" fn(*mut Self, *const Guid, *mut usize, *mut u8) -> Status,
}

#[repr(C)]
struct GraphicsOutputProtocol {
    query_mode: usize,
    set_mode: usize,
    blt: usize,
    mode: *mut GraphicsOutputMode,
}

#[repr(C)]
struct GraphicsOutputMode {
    max_mode: u32,
    mode: u32,
    info: *mut GraphicsOutputModeInfo,
    size_of_info: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct GraphicsOutputModeInfo {
    version: u32,
    horizontal_resolution: u32,
    vertical_resolution: u32,
    pixel_format: u32,
    pixel_information: [u32; 4],
    pixels_per_scan_line: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BootInfo {
    pub magic: u64,
    pub version: u32,
    pub memory_map: u64,
    pub memory_map_size: usize,
    pub descriptor_size: usize,
    pub descriptor_version: u32,
    pub framebuffer_base: u64,
    pub framebuffer_size: u64,
    pub framebuffer_width: u32,
    pub framebuffer_height: u32,
    pub pixels_per_scan_line: u32,
    pub pixel_format: u32,
    pub shell_image: u64,
    pub shell_image_size: usize,
    pub kernel_start: u64,
    pub kernel_end: u64,
    pub rsdp: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64Header {
    ident: [u8; 16],
    kind: u16,
    machine: u16,
    version: u32,
    entry: u64,
    program_header_offset: u64,
    section_header_offset: u64,
    flags: u32,
    header_size: u16,
    program_header_size: u16,
    program_header_count: u16,
    section_header_size: u16,
    section_header_count: u16,
    section_name_index: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64ProgramHeader {
    kind: u32,
    flags: u32,
    offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

#[repr(C)]
struct Allocation {
    base: u64,
    pages: usize,
}

unsafe fn allocate_pages(
    services: &BootServices,
    policy: u32,
    pages: usize,
    address: &mut u64,
) -> Result<Allocation, Status> {
    let status = (services.allocate_pages)(policy, EFI_LOADER_DATA, pages, address);
    if status != EFI_SUCCESS {
        return Err(status);
    }
    Ok(Allocation {
        base: *address,
        pages,
    })
}

unsafe fn read_file(
    services: &BootServices,
    root: *mut FileProtocol,
    path: &[u16],
) -> Result<(*mut u8, usize, Allocation), Status> {
    let mut file = core::ptr::null_mut();
    let status = ((*root).open)(root, &mut file, path.as_ptr(), FILE_MODE_READ, 0);
    if status != EFI_SUCCESS {
        return Err(status);
    }

    let mut info_buffer = [0u8; 512];
    let mut info_size = info_buffer.len();
    let status = ((*file).get_info)(
        file,
        &FILE_INFO_GUID,
        &mut info_size,
        info_buffer.as_mut_ptr(),
    );
    if status != EFI_SUCCESS || info_size < 16 {
        ((*file).close)(file);
        return Err(status);
    }
    let file_size = read_unaligned(info_buffer.as_ptr().add(8).cast::<u64>()) as usize;
    if file_size == 0 || file_size > 64 * 1024 * 1024 {
        ((*file).close)(file);
        return Err(EFI_ERROR_BIT | 2);
    }

    let pages = (file_size + 4095) / 4096;
    let mut address = u64::MAX;
    let allocation = match allocate_pages(services, ALLOCATE_ANY_PAGES, pages, &mut address) {
        Ok(a) => a,
        Err(e) => {
            ((*file).close)(file);
            return Err(e);
        }
    };
    let mut read_size = file_size;
    let status = ((*file).read)(file, &mut read_size, allocation.base as *mut u8);
    ((*file).close)(file);
    if status != EFI_SUCCESS || read_size != file_size {
        (services.free_pages)(allocation.base, allocation.pages);
        return Err(status);
    }
    Ok((allocation.base as *mut u8, file_size, allocation))
}

unsafe fn load_kernel(
    services: &BootServices,
    bytes: *const u8,
    length: usize,
) -> Result<(u64, u64, u64), Status> {
    if length < core::mem::size_of::<Elf64Header>() {
        return Err(EFI_ERROR_BIT | 3);
    }
    let header = read_unaligned(bytes.cast::<Elf64Header>());
    if &header.ident[..4] != b"\x7fELF"
        || header.ident[4] != 2
        || header.ident[5] != 1
        || header.machine != 0x3e
        || header.kind != 2
    {
        return Err(EFI_ERROR_BIT | 3);
    }
    if header.program_header_size as usize != core::mem::size_of::<Elf64ProgramHeader>() {
        return Err(EFI_ERROR_BIT | 3);
    }
    let table_end = header.program_header_offset as usize
        + header.program_header_count as usize * header.program_header_size as usize;
    if table_end > length {
        return Err(EFI_ERROR_BIT | 3);
    }

    let mut low = u64::MAX;
    let mut high = 0u64;
    for i in 0..header.program_header_count as usize {
        let offset =
            header.program_header_offset as usize + i * header.program_header_size as usize;
        let ph = read_unaligned(bytes.add(offset).cast::<Elf64ProgramHeader>());
        if ph.kind != 1 || ph.memory_size == 0 {
            continue;
        }
        if ph.file_size > ph.memory_size
            || ph.offset + ph.file_size > length as u64
            || ph.physical_address >= KERNEL_LIMIT
            || ph.physical_address + ph.memory_size > KERNEL_LIMIT
        {
            return Err(EFI_ERROR_BIT | 3);
        }
        let start = ph.physical_address & !4095;
        let end = (ph.physical_address + ph.memory_size + 4095) & !4095;
        let mut address = start;
        let pages = ((end - start) / 4096) as usize;
        allocate_pages(services, ALLOCATE_ADDRESS, pages, &mut address)?;
        write_bytes(start as *mut u8, 0, (end - start) as usize);
        copy_nonoverlapping(
            bytes.add(ph.offset as usize),
            ph.physical_address as *mut u8,
            ph.file_size as usize,
        );
        low = low.min(start);
        high = high.max(end);
    }
    if low == u64::MAX || header.entry < low || header.entry >= high {
        return Err(EFI_ERROR_BIT | 3);
    }
    Ok((header.entry, low, high))
}

unsafe fn get_framebuffer(services: &BootServices, info: &mut BootInfo) {
    let mut interface = core::ptr::null_mut();
    if (services.locate_protocol)(&GOP_GUID, core::ptr::null_mut(), &mut interface) != EFI_SUCCESS
        || interface.is_null()
    {
        return;
    }
    let gop = &*(interface as *mut GraphicsOutputProtocol);
    if gop.mode.is_null() || (*gop.mode).info.is_null() {
        return;
    }
    let mode = &*gop.mode;
    let mode_info = &*mode.info;
    info.framebuffer_base = mode.framebuffer_base;
    info.framebuffer_size = mode.framebuffer_size as u64;
    info.framebuffer_width = mode_info.horizontal_resolution;
    info.framebuffer_height = mode_info.vertical_resolution;
    info.pixels_per_scan_line = mode_info.pixels_per_scan_line;
    info.pixel_format = mode_info.pixel_format;
}

unsafe fn find_rsdp(system_table: &SystemTable) -> u64 {
    if system_table.configuration_table.is_null() {
        return 0;
    }
    for i in 0..system_table.configuration_table_entries {
        let item = &*system_table.configuration_table.add(i);
        let guid = item.vendor_guid;
        if (guid.a == 0x8868e871
            && guid.b == 0xe4f1
            && guid.c == 0x11d3
            && guid.d == [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81])
            || (guid.a == 0xeb9d2d30
                && guid.b == 0x2d88
                && guid.c == 0x11d3
                && guid.d == [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d])
        {
            return item.vendor_table as u64;
        }
    }
    0
}

unsafe fn firmware_print(system_table: *mut SystemTable, message: &[u16]) {
    if system_table.is_null() {
        return;
    }
    let output = (*system_table).console_out as *mut SimpleTextOutputProtocol;
    if !output.is_null() {
        ((*output).output_string)(output, message.as_ptr());
    }
}

unsafe fn firmware_report_status(system_table: *mut SystemTable, status: Status) {
    const PREFIX: &[u16] = &[
        13, 10, 83, 66, 79, 83, 32, 69, 70, 73, 32, 108, 111, 97, 100, 101, 114, 32, 102, 97, 105,
        108, 101, 100, 58, 32, 48, 120,
    ];
    const HEX: [u16; 16] = [
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 97, 98, 99, 100, 101, 102,
    ];
    let mut message = [0u16; PREFIX.len() + usize::BITS as usize / 4 + 3];
    message[..PREFIX.len()].copy_from_slice(PREFIX);
    let mut used = PREFIX.len();
    for shift in (0..usize::BITS).step_by(4).rev() {
        message[used] = HEX[((status >> shift) & 0xf) as usize];
        used += 1;
    }
    message[used] = 13;
    used += 1;
    message[used] = 10;
    used += 1;
    message[used] = 0;
    firmware_print(system_table, &message[..=used]);
}

unsafe fn boot(image: Handle, system_table: *mut SystemTable) -> Result<(), Status> {
    let st = &mut *system_table;
    let bs = &*st.boot_services;
    let mut loaded_raw = core::ptr::null_mut();
    let status = (bs.open_protocol)(
        image,
        &LOADED_IMAGE_GUID,
        &mut loaded_raw,
        image,
        core::ptr::null_mut(),
        OPEN_PROTOCOL_GET_PROTOCOL,
    );
    if status != EFI_SUCCESS {
        return Err(status);
    }
    let loaded = &*(loaded_raw as *mut LoadedImageProtocol);
    let mut fs_raw = core::ptr::null_mut();
    let status = (bs.open_protocol)(
        loaded.device_handle,
        &SIMPLE_FS_GUID,
        &mut fs_raw,
        image,
        core::ptr::null_mut(),
        OPEN_PROTOCOL_GET_PROTOCOL,
    );
    if status != EFI_SUCCESS {
        return Err(status);
    }
    let mut root = core::ptr::null_mut();
    let status = ((*(fs_raw as *mut FileSystemProtocol)).open_volume)(
        fs_raw as *mut FileSystemProtocol,
        &mut root,
    );
    if status != EFI_SUCCESS {
        return Err(status);
    }

    let kernel_path = [
        b'\\' as u16,
        b'k' as u16,
        b'e' as u16,
        b'r' as u16,
        b'n' as u16,
        b'e' as u16,
        b'l' as u16,
        b'.' as u16,
        b'e' as u16,
        b'l' as u16,
        b'f' as u16,
        0,
    ];
    let shell_path = [
        b'\\' as u16,
        b's' as u16,
        b'h' as u16,
        b'e' as u16,
        b'l' as u16,
        b'l' as u16,
        b'.' as u16,
        b'e' as u16,
        b'l' as u16,
        b'f' as u16,
        0,
    ];
    let (kernel_file, kernel_len, kernel_alloc) = read_file(bs, root, &kernel_path)?;
    let kernel = load_kernel(bs, kernel_file, kernel_len)?;
    (bs.free_pages)(kernel_alloc.base, kernel_alloc.pages);

    let (shell_file, shell_len, shell_staging) = read_file(bs, root, &shell_path)?;
    let shell_pages = shell_staging.pages;
    let mut shell_low = BOOT_DATA_MAX;
    let shell_allocation = allocate_pages(bs, ALLOCATE_MAX_ADDRESS, shell_pages, &mut shell_low)?;
    copy_nonoverlapping(shell_file, shell_allocation.base as *mut u8, shell_len);
    (bs.free_pages)(shell_staging.base, shell_staging.pages);

    let mut info = BootInfo {
        magic: BOOT_MAGIC,
        version: 1,
        memory_map: 0,
        memory_map_size: 0,
        descriptor_size: 0,
        descriptor_version: 0,
        framebuffer_base: 0,
        framebuffer_size: 0,
        framebuffer_width: 0,
        framebuffer_height: 0,
        pixels_per_scan_line: 0,
        pixel_format: 0,
        shell_image: shell_allocation.base,
        shell_image_size: shell_len,
        kernel_start: kernel.1,
        kernel_end: kernel.2,
        rsdp: find_rsdp(st),
    };
    get_framebuffer(bs, &mut info);
    let mut info_address = BOOT_DATA_MAX;
    let _info_allocation = allocate_pages(bs, ALLOCATE_MAX_ADDRESS, 1, &mut info_address)?;
    (info_address as *mut BootInfo).write(info);

    // Reserve a generously sized, low physical buffer before the final map query.
    let mut map_address = BOOT_DATA_MAX;
    let map_allocation = allocate_pages(bs, ALLOCATE_MAX_ADDRESS, 128, &mut map_address)?;
    let info_ptr = info_address as *mut BootInfo;
    let mut map_size = map_allocation.pages * 4096;
    let mut map_key = 0usize;
    let mut descriptor_size = 0usize;
    let mut descriptor_version = 0u32;
    let status = (bs.get_memory_map)(
        &mut map_size,
        map_address as *mut MemoryDescriptor,
        &mut map_key,
        &mut descriptor_size,
        &mut descriptor_version,
    );
    if status != EFI_SUCCESS || descriptor_size < core::mem::size_of::<MemoryDescriptor>() {
        return Err(status);
    }
    (*info_ptr).memory_map = map_address;
    (*info_ptr).memory_map_size = map_size;
    (*info_ptr).descriptor_size = descriptor_size;
    (*info_ptr).descriptor_version = descriptor_version;

    // Nothing may allocate or touch boot services between this map snapshot and ExitBootServices.
    let exit_status = (bs.exit_boot_services)(image, map_key);
    if exit_status != EFI_SUCCESS {
        let mut retry_size = map_allocation.pages * 4096;
        let status = (bs.get_memory_map)(
            &mut retry_size,
            map_address as *mut MemoryDescriptor,
            &mut map_key,
            &mut descriptor_size,
            &mut descriptor_version,
        );
        if status != EFI_SUCCESS {
            return Err(status);
        }
        (*info_ptr).memory_map_size = retry_size;
        (*info_ptr).descriptor_size = descriptor_size;
        (*info_ptr).descriptor_version = descriptor_version;
        let status = (bs.exit_boot_services)(image, map_key);
        if status != EFI_SUCCESS {
            return Err(status);
        }
    }

    let entry = kernel.0;
    asm!("mov rdi, {bootinfo}", "jmp {entry}", bootinfo = in(reg) info_ptr, entry = in(reg) entry, options(noreturn));
}

#[no_mangle]
extern "efiapi" fn efi_main(image: Handle, system_table: *mut SystemTable) -> Status {
    unsafe {
        firmware_print(
            system_table,
            &[
                83, 66, 79, 83, 32, 69, 70, 73, 32, 108, 111, 97, 100, 101, 114, 13, 10, 0,
            ],
        );
    }
    match unsafe { boot(image, system_table) } {
        Ok(()) => EFI_SUCCESS,
        Err(status) => {
            unsafe {
                firmware_report_status(system_table, status);
            }
            status
        }
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack));
        }
    }
}
