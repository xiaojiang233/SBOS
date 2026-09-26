use core::arch::asm;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
pub const USER_DATA: u16 = 0x1b;
pub const USER_CODE: u16 = 0x23;
const TSS_SELECTOR: u16 = 0x28;

#[repr(C, packed)]
struct TaskStateSegment {
    reserved0: u32,
    rsp: [u64; 3],
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    io_map_base: u16,
}

static mut GDT: [u64; 7] = [0; 7];
static mut TSS: TaskStateSegment = TaskStateSegment {
    reserved0: 0,
    rsp: [0; 3],
    reserved1: 0,
    ist: [0; 7],
    reserved2: 0,
    reserved3: 0,
    io_map_base: core::mem::size_of::<TaskStateSegment>() as u16,
};

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub unsafe fn init(interrupt_stack_top: u64) {
    GDT[0] = 0;
    GDT[1] = 0x00af_9a00_0000_ffff; // 64-bit ring-0 code
    GDT[2] = 0x00cf_9200_0000_ffff; // ring-0 data
    GDT[3] = 0x00cf_f200_0000_ffff; // ring-3 data
    GDT[4] = 0x00af_fa00_0000_ffff; // 64-bit ring-3 code
    core::ptr::addr_of_mut!(TSS.rsp[0]).write_unaligned(interrupt_stack_top);
    core::ptr::addr_of_mut!(TSS.io_map_base)
        .write_unaligned(core::mem::size_of::<TaskStateSegment>() as u16);
    let base = core::ptr::addr_of!(TSS) as u64;
    let limit = (core::mem::size_of::<TaskStateSegment>() - 1) as u64;
    GDT[5] = (limit & 0xffff)
        | ((base & 0x00ff_ffff) << 16)
        | (0x89u64 << 40)
        | (((limit >> 16) & 0xf) << 48)
        | (((base >> 24) & 0xff) << 56);
    GDT[6] = base >> 32;
    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of::<[u64; 7]>() - 1) as u16,
        base: core::ptr::addr_of!(GDT) as u64,
    };
    asm!(
        "lgdt [{ptr}]",
        "mov ax, {data}",
        "mov ds, ax", "mov es, ax", "mov ss, ax",
        "push {code}",
        "lea rax, [rip + 2f]",
        "push rax",
        "retfq",
        "2:",
        "mov ax, {tss}",
        "ltr ax",
        ptr = in(reg) &pointer,
        data = const KERNEL_DATA,
        code = const KERNEL_CODE,
        tss = const TSS_SELECTOR,
        out("rax") _,
        options(preserves_flags)
    );
}

pub unsafe fn set_ring0_stack(stack_top: u64) {
    core::ptr::addr_of_mut!(TSS.rsp[0]).write_unaligned(stack_top);
}
