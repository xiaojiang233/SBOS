#ifndef _SBOS_SYS_WAIT_H
#define _SBOS_SYS_WAIT_H
#include <sys/types.h>
#ifndef _POSIX_VERSION
#define _POSIX_VERSION 200809L
#endif
pid_t waitpid(pid_t pid, int *status, int options);
#define WNOHANG 1
#define WUNTRACED 2
#define WCONTINUED 8
#define WIFEXITED(status) (((status) & 0x7f) == 0)
#define WEXITSTATUS(status) (((status) >> 8) & 0xff)
#define WIFSIGNALED(status) (!WIFSTOPPED(status) && !WIFEXITED(status))
#define WTERMSIG(status) ((status) & 0x7f)
#define WIFSTOPPED(status) (((status) & 0xff) == 0x7f)
#define WSTOPSIG(status) (((status) >> 8) & 0xff)
#define WIFCONTINUED(status) ((status) == 0xffff)
#endif
