#include <errno.h>
#include <internal/syscall.h>
#include <stdio.h>
#include <string.h>
#include <stdint.h>

int rename(const char *old_path, const char *new_path) {
    size_t old_length;
    size_t new_length;
    if (old_path == 0 || new_path == 0) {
        errno = EFAULT;
        return -1;
    }
    old_length = strnlen(old_path, 513);
    new_length = strnlen(new_path, 513);
    if (old_length == 0 || new_length == 0) { errno = ENOENT; return -1; }
    if (old_length > 512 || new_length > 512) { errno = ENAMETOOLONG; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_RENAME, (uint64_t)(uintptr_t)old_path, old_length,
        (uint64_t)(uintptr_t)new_path, new_length, 0, 0));
}
