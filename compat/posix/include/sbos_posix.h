#ifndef SBOS_POSIX_H
#define SBOS_POSIX_H

#include <stddef.h>
#include <stdint.h>

/* File descriptors 0, 1, and 2 are stdin, stdout, and stderr. */
intptr_t sbos_posix_open(const char *path, uint64_t rights);
intptr_t sbos_posix_read(int fd, void *buffer, size_t length);
intptr_t sbos_posix_write(int fd, const void *buffer, size_t length);
intptr_t sbos_posix_close(int fd);
intptr_t sbos_posix_getpid(void);
intptr_t sbos_posix_fork(void); /* returns -ENOSYS until fork semantics exist */

#endif
