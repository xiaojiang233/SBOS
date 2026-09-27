#include <errno.h>
#include <internal/syscall.h>
#include <netinet/in.h>
#include <stdint.h>
#include <sys/socket.h>

int socket(int domain, int type, int protocol) {
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_SOCKET, (uint64_t)(uint32_t)domain,
        (uint64_t)(uint32_t)type, (uint64_t)(uint32_t)protocol, 0, 0, 0));
}

int bind(int socket_fd, const struct sockaddr *address, socklen_t address_length) {
    if (address == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_BIND, (uint64_t)(uint32_t)socket_fd,
        (uint64_t)(uintptr_t)address, address_length, 0, 0, 0));
}

int connect(int socket_fd, const struct sockaddr *address, socklen_t address_length) {
    if (address == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_CONNECT, (uint64_t)(uint32_t)socket_fd,
        (uint64_t)(uintptr_t)address, address_length, 0, 0, 0));
}

ssize_t sendto(int socket_fd, const void *buffer, size_t length, int flags,
               const struct sockaddr *destination, socklen_t destination_length) {
    if (length != 0 && buffer == 0) { errno = EFAULT; return -1; }
    if (destination != 0 && destination_length < sizeof(struct sockaddr_in)) {
        errno = EINVAL;
        return -1;
    }
    return (ssize_t)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_SENDTO, (uint64_t)(uint32_t)socket_fd,
        (uint64_t)(uintptr_t)buffer, length, (uint64_t)(uint32_t)flags,
        (uint64_t)(uintptr_t)destination, destination_length));
}

ssize_t recvfrom(int socket_fd, void *buffer, size_t length, int flags,
                 struct sockaddr *source, socklen_t *source_length) {
    if (length != 0 && buffer == 0) { errno = EFAULT; return -1; }
    if ((source == 0) != (source_length == 0)) { errno = EFAULT; return -1; }
    return (ssize_t)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_RECVFROM, (uint64_t)(uint32_t)socket_fd,
        (uint64_t)(uintptr_t)buffer, length, (uint64_t)(uint32_t)flags,
        (uint64_t)(uintptr_t)source, (uint64_t)(uintptr_t)source_length));
}

ssize_t send(int socket_fd, const void *buffer, size_t length, int flags) {
    return sendto(socket_fd, buffer, length, flags, 0, 0);
}

ssize_t recv(int socket_fd, void *buffer, size_t length, int flags) {
    return recvfrom(socket_fd, buffer, length, flags, 0, 0);
}

uint16_t htons(uint16_t value) {
    return (uint16_t)((value << 8) | (value >> 8));
}

uint16_t ntohs(uint16_t value) { return htons(value); }

uint32_t htonl(uint32_t value) {
    return ((value & 0x000000ffu) << 24) | ((value & 0x0000ff00u) << 8) |
           ((value & 0x00ff0000u) >> 8) | ((value & 0xff000000u) >> 24);
}

uint32_t ntohl(uint32_t value) { return htonl(value); }
