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
pub struct MemoryDescriptor {
    pub memory_type: u32,
    pub _padding: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attributes: u64,
}

pub const BOOT_MAGIC: u64 = 0x5342_4f53_424f_4f54;
