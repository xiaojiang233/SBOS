.section .text.entry,"ax"
.global _start
.type _start,@function
_start:
    xorq %rbp, %rbp
    movq %rsp, %rbx
    movq (%rbx), %r12
    leaq 8(%rbx), %r13
    leaq 16(%rbx,%r12,8), %r14
    movq (%r13), %rdi
    call __sbos_init_program_name
    movq %r14, %rdi
    call __sbos_init_environment
    movq %r12, %rdi
    movq %r13, %rsi
    movq %r14, %rdx
    andq $-16, %rsp
    call main
    movl %eax, %edi
    movl $4, %eax
    int $0x80
1:
    pause
    jmp 1b
.size _start, .-_start
.extern __sbos_init_environment
.extern __sbos_init_program_name
