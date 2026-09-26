#ifndef _SBOS_INTERNAL_SYSCALL_H
#define _SBOS_INTERNAL_SYSCALL_H
#include <stdint.h>
#include <sbos/native.h>

int64_t __sbos_syscall6(uint64_t number, uint64_t a0, uint64_t a1,
                        uint64_t a2, uint64_t a3, uint64_t a4,
                        uint64_t a5);
int __sbos_errno_from_result(int64_t result);
int64_t __sbos_checked_result(int64_t result);
int64_t __sbos_posix_checked_result(int64_t result);
uint32_t __sbos_fd_handle(int fd);

#endif
