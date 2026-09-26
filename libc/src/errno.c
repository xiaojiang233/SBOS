#include <errno.h>
#include <internal/syscall.h>

static int current_errno;

int *__errno_location(void) { return &current_errno; }

int __sbos_errno_from_result(int64_t result) {
    switch (result) {
    case -1: return EINVAL;
    case -2: return ENOENT;
    case -3: return EACCES;
    case -4: return ENOMEM;
    case -5: return ENOSYS;
    case -6: return EAGAIN;
    case -7: return ETIMEDOUT;
    case -8: return EFAULT;
    case -9: return ECHILD;
    case -10: return EPIPE;
    default: return EIO;
    }
}

int64_t __sbos_checked_result(int64_t result) {
    if (result < 0) {
        errno = __sbos_errno_from_result(result);
        return -1;
    }
    return result;
}

int64_t __sbos_posix_checked_result(int64_t result) {
    if (result < 0) {
        errno = (int)-result;
        return -1;
    }
    return result;
}
