use core::arch::{asm, global_asm};
use core::mem::size_of;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub error: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        attributes: 0,
        offset_mid: 0,
        offset_high: 0,
        reserved: 0,
    };
    fn new(handler: u64, dpl3: bool) -> Self {
        Self {
            offset_low: handler as u16,
            selector: 0x08,
            ist: 0,
            attributes: if dpl3 { 0xee } else { 0x8e },
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            reserved: 0,
        }
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

static mut IDT: [IdtEntry; 256] = [IdtEntry::MISSING; 256];

#[repr(align(16))]
struct IdleStack([u8; 16 * 1024]);
static mut IDLE_STACK: IdleStack = IdleStack([0; 16 * 1024]);

fn idle_loop() -> ! {
    loop {
        unsafe { asm!("sti", "hlt", options(nomem, nostack)); }
    }
}

/// Build a ring-0 interrupt frame on a dedicated stack for the no-runnable
/// thread case. Timer interrupts can wake a Ready thread from this idle frame.
pub fn idle_frame() -> *mut TrapFrame {
    unsafe {
        let top = (core::ptr::addr_of_mut!(IDLE_STACK) as *mut u8)
            .add(core::mem::size_of::<IdleStack>());
        let frame = top.sub(core::mem::size_of::<TrapFrame>()).cast::<TrapFrame>();
        frame.write(TrapFrame {
            rip: idle_loop as usize as u64,
            cs: crate::arch::x86_64::gdt::KERNEL_CODE as u64,
            rflags: 0x202,
            rsp: top as u64,
            ss: crate::arch::x86_64::gdt::KERNEL_DATA as u64,
            ..TrapFrame::default()
        });
        frame
    }
}

extern "C" {
    fn isr0();
    fn isr1();
    fn isr2();
    fn isr3();
    fn isr4();
    fn isr5();
    fn isr6();
    fn isr7();
    fn isr8();
    fn isr9();
    fn isr10();
    fn isr11();
    fn isr12();
    fn isr13();
    fn isr14();
    fn isr15();
    fn isr16();
    fn isr17();
    fn isr18();
    fn isr19();
    fn isr20();
    fn isr21();
    fn isr22();
    fn isr23();
    fn isr24();
    fn isr25();
    fn isr26();
    fn isr27();
    fn isr28();
    fn isr29();
    fn isr30();
    fn isr31();
    fn isr32();
    fn isr36();
    fn isr128();
}

global_asm!(
    r#"
.section .text
.macro ISR_NOERR n
    .global isr\n
isr\n:
    push 0
    push \n
    jmp isr_common
.endm
.macro ISR_ERR n
    .global isr\n
isr\n:
    push \n
    jmp isr_common
.endm
ISR_NOERR 0
ISR_NOERR 1
ISR_NOERR 2
ISR_NOERR 3
ISR_NOERR 4
ISR_NOERR 5
ISR_NOERR 6
ISR_NOERR 7
ISR_ERR 8
ISR_NOERR 9
ISR_ERR 10
ISR_ERR 11
ISR_ERR 12
ISR_ERR 13
ISR_ERR 14
ISR_NOERR 15
ISR_NOERR 16
ISR_ERR 17
ISR_NOERR 18
ISR_NOERR 19
ISR_NOERR 20
ISR_ERR 21
ISR_NOERR 22
ISR_NOERR 23
ISR_NOERR 24
ISR_NOERR 25
ISR_NOERR 26
ISR_NOERR 27
ISR_NOERR 28
ISR_ERR 29
ISR_ERR 30
ISR_NOERR 31
ISR_NOERR 32
ISR_NOERR 36
ISR_NOERR 128

.global isr_common
isr_common:
    push rax
    push rbx
    push rcx
    push rdx
    push rbp
    push rsi
    push rdi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi, rsp
    cld
    and rsp, -16
    call trap_dispatch
    mov rsp, rax
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rdi
    pop rsi
    pop rbp
    pop rdx
    pop rcx
    pop rbx
    pop rax
    add rsp, 16
    iretq
"#
);

pub unsafe fn init() {
    let handlers: [unsafe extern "C" fn(); 32] = [
        isr0, isr1, isr2, isr3, isr4, isr5, isr6, isr7, isr8, isr9, isr10, isr11, isr12, isr13,
        isr14, isr15, isr16, isr17, isr18, isr19, isr20, isr21, isr22, isr23, isr24, isr25, isr26,
        isr27, isr28, isr29, isr30, isr31,
    ];
    let idt = core::ptr::addr_of_mut!(IDT);
    for (index, handler) in handlers.iter().enumerate() {
        (*idt)[index] = IdtEntry::new(*handler as usize as u64, false);
    }
    (*idt)[32] = IdtEntry::new(isr32 as usize as u64, false);
    (*idt)[36] = IdtEntry::new(isr36 as usize as u64, false);
    (*idt)[128] = IdtEntry::new(isr128 as usize as u64, true);
    let descriptor = Idtr {
        limit: (size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: idt as u64,
    };
    asm!("lidt [{0}]", in(reg) &descriptor, options(readonly, nostack, preserves_flags));
}
