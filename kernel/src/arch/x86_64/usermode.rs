use core::arch::asm;

pub unsafe fn enter(entry: u64, stack: u64) -> ! {
    // ELF process entry receives argc at [rsp]. C crt0 aligns rsp itself before
    // making a normal function call, so do not apply the function-entry offset
    // here.
    let user_stack = stack;
    asm!(
        "push {user_ss}",
        "push {user_stack}",
        "push 0x202",
        "push {user_cs}",
        "push {entry}",
        "iretq",
        user_ss = const super::gdt::USER_DATA,
        user_stack = in(reg) user_stack,
        user_cs = const super::gdt::USER_CODE,
        entry = in(reg) entry,
        options(noreturn)
    )
}
