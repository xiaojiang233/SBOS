#include <errno.h>
#include <stdint.h>
#include <internal/syscall.h>
#include <sys/mman.h>

int64_t __sbos_syscall6(uint64_t number, uint64_t a0, uint64_t a1,
                        uint64_t a2, uint64_t a3, uint64_t a4,
                        uint64_t a5) {
    register uint64_t r10 __asm__("r10") = a3;
    register uint64_t r8 __asm__("r8") = a4;
    register uint64_t r9 __asm__("r9") = a5;
    __asm__ volatile("int $0x80"
                     : "+a"(number)
                     : "D"(a0), "S"(a1), "d"(a2), "r"(r10), "r"(r8), "r"(r9)
                     : "rcx", "r11", "memory", "cc");
    return (int64_t)number;
}

void *mmap(void *address, size_t length, int protection, int flags, int fd,
           long offset) {
    uint64_t permissions = 0;
    int64_t result;
    if (length == 0 || flags != (MAP_PRIVATE | MAP_ANONYMOUS) ||
        fd != -1 || offset != 0 || (protection & ~(PROT_READ | PROT_WRITE | PROT_EXEC)) != 0 ||
        (protection & PROT_READ) == 0) {
        errno = EINVAL;
        return MAP_FAILED;
    }
    if (protection & PROT_READ) permissions |= 1;
    if (protection & PROT_WRITE) permissions |= 2;
    if (protection & PROT_EXEC) permissions |= 4;
    result = __sbos_syscall6(SBOS_MEMORY_MAP, (uint64_t)(uintptr_t)address,
                             length, permissions, 0, 0, 0);
    result = __sbos_checked_result(result);
    return result < 0 ? MAP_FAILED : (void *)(uintptr_t)result;
}

int munmap(void *address, size_t length) {
    int64_t result = __sbos_syscall6(SBOS_MEMORY_UNMAP,
                                     (uint64_t)(uintptr_t)address, length,
                                     0, 0, 0, 0);
    return (int)__sbos_checked_result(result);
}
