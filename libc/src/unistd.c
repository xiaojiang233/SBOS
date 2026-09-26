#include <errno.h>
#include <fcntl.h>
#include <internal/syscall.h>
#include <limits.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#define IO_LIMIT 4096

static size_t path_length(const char *path) { return strnlen(path, 513); }

ssize_t read(int fd, void *buffer, size_t length) {
    if (length != 0 && buffer == 0) { errno = EFAULT; return -1; }
    return (ssize_t)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_READ, (uint64_t)(uint32_t)fd,
        (uint64_t)(uintptr_t)buffer, length, 0, 0, 0));
}

ssize_t write(int fd, const void *buffer, size_t length) {
    size_t done = 0;
    if (length != 0 && buffer == 0) { errno = EFAULT; return -1; }
    while (done < length) {
        size_t amount = length - done;
        int64_t result;
        if (amount > IO_LIMIT) amount = IO_LIMIT;
        result = __sbos_posix_checked_result(__sbos_syscall6(
            SBOS_POSIX_WRITE, (uint64_t)(uint32_t)fd,
            (uint64_t)((uintptr_t)buffer + done), amount, 0, 0, 0));
        if (result < 0) return done != 0 ? (ssize_t)done : -1;
        if (result == 0) break;
        done += (size_t)result;
        if ((size_t)result < amount) break;
    }
    return (ssize_t)done;
}

int open(const char *path, int flags, ...) {
    size_t length;
    if (path == 0) { errno = EFAULT; return -1; }
    length = path_length(path);
    if (length > 512) { errno = ENAMETOOLONG; return -1; }
    if ((flags & ~(O_ACCMODE | O_CLOEXEC | O_CREAT | O_EXCL | O_TRUNC | O_APPEND)) != 0) {
        errno = ENOSYS;
        return -1;
    }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_OPEN, (uint64_t)(uintptr_t)path, length,
        (uint64_t)(uint32_t)flags, 0, 0, 0));
}

int creat(const char *path, mode_t mode) {
    (void)mode; /* SBFS takes new file permissions from the parent ACL */
    return open(path, O_WRONLY | O_CREAT | O_TRUNC);
}

int close(int fd) {
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_CLOSE, (uint64_t)(uint32_t)fd, 0, 0, 0, 0, 0));
}

int gethostname(char *name, size_t length) {
    static const char hostname[] = "sbos";
    if (name == 0) { errno = EFAULT; return -1; }
    if (length < sizeof(hostname)) { errno = ENAMETOOLONG; return -1; }
    memcpy(name, hostname, sizeof(hostname));
    return 0;
}

int unlink(const char *path) {
    size_t length;
    if (path == 0) { errno = EFAULT; return -1; }
    length = path_length(path);
    if (length > 512) { errno = ENAMETOOLONG; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_UNLINK, (uint64_t)(uintptr_t)path, length, 0, 0, 0, 0));
}

int dup(int fd) {
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_DUP, (uint64_t)(uint32_t)fd, 0, 0, 0, 0, 0));
}

int dup2(int source, int destination) {
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_DUP2, (uint64_t)(uint32_t)source,
        (uint64_t)(uint32_t)destination, 0, 0, 0, 0));
}

int fcntl(int fd, int command, ...) {
    int argument = 0;
    va_list arguments;
    if (command != F_GETFD && command != F_GETFL) {
        va_start(arguments, command);
        argument = va_arg(arguments, int);
        va_end(arguments);
    }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_FCNTL, (uint64_t)(uint32_t)fd,
        (uint64_t)(uint32_t)command, (uint64_t)(uint32_t)argument,
        0, 0, 0));
}

off_t lseek(int fd, off_t offset, int whence) {
    return (off_t)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_LSEEK, (uint64_t)(uint32_t)fd, (uint64_t)offset,
        (uint64_t)(uint32_t)whence, 0, 0, 0));
}

int pipe(int descriptors[2]) {
    if (descriptors == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_PIPE, (uint64_t)(uintptr_t)descriptors, 0, 0, 0, 0, 0));
}

pid_t getpid(void) {
    return (pid_t)__sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_ID, 0, 0, 0, 0, 0, 0));
}
pid_t getppid(void) {
    return (pid_t)__sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_PARENT_ID, 0, 0, 0, 0, 0, 0));
}
uid_t getuid(void) {
    return (uid_t)__sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_UID, 0, 0, 0, 0, 0, 0));
}
uid_t geteuid(void) { return getuid(); }
gid_t getgid(void) {
    return (gid_t)__sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_GID, 0, 0, 0, 0, 0, 0));
}
gid_t getegid(void) { return getgid(); }
int setuid(uid_t uid) {
    if (uid == getuid()) return 0;
    errno = EPERM;
    return -1;
}
int setgid(gid_t gid) {
    if (gid == getgid()) return 0;
    errno = EPERM;
    return -1;
}

pid_t fork(void) {
    return (pid_t)__sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_FORK, 0, 0, 0, 0, 0, 0));
}

int execve(const char *path, char *const argv[], char *const envp[]) {
    if (path == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_EXECVE, (uint64_t)(uintptr_t)path,
        (uint64_t)(uintptr_t)argv, (uint64_t)(uintptr_t)envp,
        0, 0, 0));
}

int chdir(const char *path) {
    size_t length;
    if (path == 0) { errno = EFAULT; return -1; }
    length = path_length(path);
    if (length > 512) { errno = ENAMETOOLONG; return -1; }
    return (int)__sbos_checked_result(__sbos_syscall6(
        SBOS_DIRECTORY_CHANGE, (uint64_t)(uintptr_t)path, length, 0, 0, 0, 0));
}

char *getcwd(char *buffer, size_t size) {
    char local[512];
    int allocated = 0;
    int64_t result;
    if (buffer == 0) {
        buffer = (char *)malloc(sizeof(local));
        if (buffer == 0) return 0;
        size = sizeof(local);
        allocated = 1;
    }
    if (size == 0) { if (allocated) free(buffer); errno = ERANGE; return 0; }
    result = __sbos_checked_result(__sbos_syscall6(
        SBOS_DIRECTORY_CURRENT, (uint64_t)(uintptr_t)local, sizeof(local), 0, 0, 0, 0));
    if (result < 0) { if (allocated) free(buffer); return 0; }
    if ((size_t)result + 1 > size) { if (allocated) free(buffer); errno = ERANGE; return 0; }
    memcpy(buffer, local, (size_t)result);
    buffer[result] = '\0';
    return buffer;
}

int isatty(int fd) {
    int64_t result = __sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_ISATTY, (uint64_t)(uint32_t)fd, 0, 0, 0, 0, 0));
    return result > 0 ? 1 : 0;
}

char *ttyname(int fd) {
    if (isatty(fd)) { errno = ENOTTY; return 0; }
    return 0;
}

unsigned int sleep(unsigned int seconds) {
    int64_t start = __sbos_checked_result(__sbos_syscall6(
        SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0));
    uint64_t duration = (uint64_t)seconds * 100;
    uint64_t target;
    if (seconds == 0) return 0;
    if (start < 0) return seconds;
    if (duration > UINT64_MAX - (uint64_t)start) { errno = EOVERFLOW; return seconds; }
    target = (uint64_t)start + duration;
    for (;;) {
        int64_t tick = __sbos_checked_result(__sbos_syscall6(
            SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0));
        if (tick < 0) return seconds;
        if ((uint64_t)tick >= target) return 0;
        __asm__ volatile("pause");
    }
}

int usleep(useconds_t microseconds) {
    uint64_t ticks = ((uint64_t)microseconds + 9999) / 10000;
    int64_t start = __sbos_checked_result(__sbos_syscall6(
        SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0));
    uint64_t target;
    if (start < 0) return -1;
    target = (uint64_t)start + ticks;
    while (ticks != 0) {
        int64_t tick = __sbos_checked_result(__sbos_syscall6(
            SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0));
        if (tick < 0) return -1;
        if ((uint64_t)tick >= target) break;
        __asm__ volatile("pause");
    }
    return 0;
}

unsigned int alarm(unsigned int seconds) {
    (void)seconds; errno = ENOSYS; return 0;
}
int pause(void) { errno = ENOSYS; return -1; }

void _exit(int status) {
    (void)__sbos_syscall6(SBOS_PROCESS_EXIT, (uint32_t)status, 0, 0, 0, 0, 0);
    for (;;) __asm__ volatile("pause");
}

int waitpid(pid_t pid, int *status, int options) {
    int64_t result;
    if ((options & ~WNOHANG) != 0) { errno = ENOSYS; return -1; }
    result = __sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_WAITPID, (uint64_t)(int64_t)pid,
        (uint64_t)(uintptr_t)status, (uint64_t)options, 0, 0, 0));
    return (pid_t)result;
}
