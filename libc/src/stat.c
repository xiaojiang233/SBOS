#include <errno.h>
#include <internal/syscall.h>
#include <stdint.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

_Static_assert(sizeof(struct stat) == 120, "SBOS stat ABI layout mismatch");

static int stat_path(const char *path, struct stat *status) {
    size_t length;
    int64_t result;
    if (path == 0 || status == 0) { errno = EFAULT; return -1; }
    length = strnlen(path, 513);
    if (length == 0) { errno = ENOENT; return -1; }
    if (length > 512) { errno = ENAMETOOLONG; return -1; }
    result = __sbos_syscall6(SBOS_STAT_PATH, (uint64_t)(uintptr_t)path,
                            length, (uint64_t)(uintptr_t)status, 0, 0, 0);
    return (int)__sbos_checked_result(result);
}

int stat(const char *path, struct stat *status) { return stat_path(path, status); }
int lstat(const char *path, struct stat *status) { return stat_path(path, status); }

int fstat(int fd, struct stat *status) {
    if (status == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_FSTAT, (uint64_t)(uint32_t)fd,
        (uint64_t)(uintptr_t)status, 0, 0, 0, 0));
}

int access(const char *path, int mode) {
    struct stat status;
    if ((mode & ~(R_OK | W_OK | X_OK)) != 0) { errno = EINVAL; return -1; }
    if (stat(path, &status) < 0) return -1;
    if ((mode & R_OK) && !(status.st_mode & (S_IRUSR | S_IRGRP | S_IROTH))) {
        errno = EACCES;
        return -1;
    }
    if ((mode & W_OK) && !(status.st_mode & (S_IWUSR | S_IWGRP | S_IWOTH))) {
        errno = EACCES;
        return -1;
    }
    if ((mode & X_OK) && !(status.st_mode & (S_IXUSR | S_IXGRP | S_IXOTH))) {
        errno = EACCES;
        return -1;
    }
    return 0;
}

int chmod(const char *path, mode_t mode) {
    (void)path;
    (void)mode;
    errno = ENOSYS;
    return -1;
}

int fchmod(int fd, mode_t mode) {
    (void)fd;
    (void)mode;
    errno = ENOSYS;
    return -1;
}

int mkdir(const char *path, mode_t mode) {
    size_t length;
    (void)mode;
    if (path == 0) { errno = EFAULT; return -1; }
    length = strnlen(path, 513);
    if (length > 512) { errno = ENAMETOOLONG; return -1; }
    return (int)__sbos_checked_result(__sbos_syscall6(
        SBOS_DIRECTORY_CREATE, (uint64_t)(uintptr_t)path, length, 0, 0, 0, 0));
}

/*
 * The process file creation mask. SBFS derives new file permissions from the
 * parent directory ACL rather than from mode bits, so the mask is recorded and
 * reported but does not restrict what a program creates.
 */
mode_t umask(mode_t mask) {
    return (mode_t)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_UMASK, (uint64_t)(mask & 0777u), 0, 0, 0, 0, 0));
}
