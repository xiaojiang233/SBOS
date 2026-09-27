#ifndef _SBOS_SYS_SELECT_H
#define _SBOS_SYS_SELECT_H

/* Target ABI types are kept freestanding so Gnulib wrappers can include this
 * header before their generated configuration headers. */
#ifndef _SBOS_TIMEVAL_DEFINED
#define _SBOS_TIMEVAL_DEFINED
struct timeval {
    long tv_sec;
    long tv_usec;
};
#endif

#define FD_SETSIZE 128
typedef struct { unsigned long long __bits[2]; } fd_set;

#define FD_ZERO(set) do { (set)->__bits[0] = 0; (set)->__bits[1] = 0; } while (0)
#define FD_SET(fd, set) ((set)->__bits[(unsigned)(fd) >> 6] |= 1ULL << ((unsigned)(fd) & 63))
#define FD_CLR(fd, set) ((set)->__bits[(unsigned)(fd) >> 6] &= ~(1ULL << ((unsigned)(fd) & 63)))
#define FD_ISSET(fd, set) (((set)->__bits[(unsigned)(fd) >> 6] >> ((unsigned)(fd) & 63)) & 1ULL)

int select(int nfds, fd_set *readfds, fd_set *writefds,
           fd_set *exceptfds, struct timeval *timeout);

#endif
