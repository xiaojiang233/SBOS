#include <dirent.h>
#include <errno.h>
#include <internal/syscall.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define DIRECTORY_BUFFER_SIZE 4096

struct __sbos_DIR {
    uint32_t handle;
    size_t length;
    size_t cursor;
    int loaded;
    char buffer[DIRECTORY_BUFFER_SIZE];
    struct dirent current;
};

DIR *opendir(const char *path) {
    int64_t raw;
    size_t length;
    DIR *directory;
    if (path == 0) { errno = EFAULT; return 0; }
    length = strnlen(path, 513);
    if (length == 0) { errno = ENOENT; return 0; }
    if (length > 512) { errno = ENAMETOOLONG; return 0; }
    raw = __sbos_checked_result(__sbos_syscall6(
        SBOS_DIRECTORY_OPEN, (uint64_t)(uintptr_t)path, length, 0, 0, 0, 0));
    if (raw < 0) return 0;
    directory = (DIR *)malloc(sizeof(*directory));
    if (directory == 0) {
        (void)__sbos_syscall6(SBOS_HANDLE_CLOSE, (uint32_t)raw, 0, 0, 0, 0, 0);
        return 0;
    }
    directory->handle = (uint32_t)raw;
    directory->length = 0;
    directory->cursor = 0;
    directory->loaded = 0;
    return directory;
}

struct dirent *readdir(DIR *directory) {
    if (directory == 0) { errno = EBADF; return 0; }
    if (!directory->loaded) {
        int64_t result = __sbos_checked_result(__sbos_syscall6(
            SBOS_DIRECTORY_READ, directory->handle,
            (uint64_t)(uintptr_t)directory->buffer,
            sizeof(directory->buffer), 0, 0, 0));
        if (result < 0) return 0;
        directory->length = (size_t)result;
        directory->cursor = 0;
        directory->loaded = 1;
    }
    while (directory->cursor < directory->length) {
        size_t start = directory->cursor;
        size_t length;
        int is_directory;
        while (directory->cursor < directory->length &&
               directory->buffer[directory->cursor] != '\n')
            ++directory->cursor;
        length = directory->cursor - start;
        if (directory->cursor < directory->length) ++directory->cursor;
        if (length == 0) continue;
        is_directory = directory->buffer[start + length - 1] == '/';
        if (is_directory) --length;
        if (length >= sizeof(directory->current.d_name)) {
            errno = ENAMETOOLONG;
            continue;
        }
        memcpy(directory->current.d_name, directory->buffer + start, length);
        directory->current.d_name[length] = '\0';
        directory->current.d_ino = 0;
        directory->current.d_off = (long)directory->cursor;
        directory->current.d_reclen = (unsigned short)sizeof(directory->current);
        directory->current.d_type = is_directory ? DT_DIR : DT_REG;
        return &directory->current;
    }
    return 0;
}

int closedir(DIR *directory) {
    int64_t result;
    if (directory == 0) { errno = EBADF; return -1; }
    result = __sbos_checked_result(__sbos_syscall6(
        SBOS_HANDLE_CLOSE, directory->handle, 0, 0, 0, 0, 0));
    free(directory);
    return (int)result;
}

void rewinddir(DIR *directory) {
    if (directory != 0) directory->cursor = 0;
}

int dirfd(DIR *directory) {
    (void)directory;
    errno = ENOSYS;
    return -1;
}
