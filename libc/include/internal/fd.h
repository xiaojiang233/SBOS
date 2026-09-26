#ifndef _SBOS_INTERNAL_FD_H
#define _SBOS_INTERNAL_FD_H
#include <stdint.h>
#define SBOS_FD_CONSOLE 0
#define SBOS_FD_FILE 1
#define SBOS_FD_PIPE_READ 2
#define SBOS_FD_PIPE_WRITE 3
uint32_t __sbos_fd_handle(int fd);
int __sbos_fd_is_open(int fd);
int __sbos_fd_kind(int fd);
#endif
