#ifndef _SBOS_SYS_MMAN_H
#define _SBOS_SYS_MMAN_H
#include <stddef.h>

#define PROT_READ 1
#define PROT_WRITE 2
#define PROT_EXEC 4
#define MAP_PRIVATE 2
#define MAP_ANONYMOUS 0x20
#define MAP_FAILED ((void *)-1)

void *mmap(void *address, size_t length, int protection, int flags, int fd, long offset);
int munmap(void *address, size_t length);

#endif
