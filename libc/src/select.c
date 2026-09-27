#include <errno.h>
#include <internal/syscall.h>
#include <sys/select.h>

int select(int nfds, fd_set *readfds, fd_set *writefds,
           fd_set *exceptfds, struct timeval *timeout) {
    int64_t result;
    if (nfds < 0 || nfds > FD_SETSIZE) { errno = EINVAL; return -1; }
    result = __sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_SELECT, (uint64_t)(uint32_t)nfds,
        (uint64_t)(uintptr_t)readfds, (uint64_t)(uintptr_t)writefds,
        (uint64_t)(uintptr_t)exceptfds, (uint64_t)(uintptr_t)timeout, 0));
    return (int)result;
}
