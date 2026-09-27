#ifndef _SBOS_SYS_SOCKET_H
#define _SBOS_SYS_SOCKET_H

#include <stdint.h>
#include <sys/types.h>

typedef uint32_t socklen_t;
typedef uint16_t sa_family_t;

struct sockaddr {
    sa_family_t sa_family;
    char sa_data[14];
};

#define AF_UNSPEC 0
#define AF_UNIX 1
#define AF_INET 2
#define AF_INET6 10
#define PF_UNSPEC AF_UNSPEC
#define PF_UNIX AF_UNIX
#define PF_INET AF_INET
#define PF_INET6 AF_INET6

#define SOCK_STREAM 1
#define SOCK_DGRAM 2
#define SOCK_RAW 3
#define SOCK_NONBLOCK 0x800
#define SOCK_CLOEXEC 0x80000

#define SOL_SOCKET 1
#define SO_ERROR 4
#define SO_TYPE 3

#define MSG_PEEK 0x02
#define MSG_DONTWAIT 0x40

int socket(int domain, int type, int protocol);
int bind(int socket_fd, const struct sockaddr *address, socklen_t address_length);
int connect(int socket_fd, const struct sockaddr *address, socklen_t address_length);
int listen(int socket_fd, int backlog);
int accept(int socket_fd, struct sockaddr *address, socklen_t *address_length);
ssize_t sendto(int socket_fd, const void *buffer, size_t length, int flags,
               const struct sockaddr *destination, socklen_t destination_length);
ssize_t recvfrom(int socket_fd, void *buffer, size_t length, int flags,
                 struct sockaddr *source, socklen_t *source_length);
ssize_t send(int socket_fd, const void *buffer, size_t length, int flags);
ssize_t recv(int socket_fd, void *buffer, size_t length, int flags);

#endif
