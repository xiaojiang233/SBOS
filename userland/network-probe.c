#include <errno.h>
#include <netdb.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

static const unsigned char dns_query[] = {
    0x53, 0x42, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 'e', 'x', 'a', 'm', 'p', 'l', 'e',
    0x03, 'c', 'o', 'm', 0x00, 0x00, 0x01, 0x00, 0x01
};

static struct sockaddr_in dns_server(void) {
    struct sockaddr_in address;
    memset(&address, 0, sizeof(address));
    address.sin_family = AF_INET;
    address.sin_port = htons(53);
    address.sin_addr.s_addr = htonl(0x0a000203u);
    return address;
}

static int response_is_dns(const unsigned char *packet, ssize_t length) {
    return length >= 12 && packet[0] == 0x53 && packet[1] == 0x42 &&
           (packet[2] & 0x80) != 0;
}

int main(void) {
    unsigned char response[512];
    struct sockaddr_in server = dns_server();
    struct sockaddr_in source;
    socklen_t source_length = sizeof(source);
    int original = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    int fd;
    ssize_t sent;
    ssize_t received;
    pid_t child;
    int child_status;
    struct addrinfo hints;
    struct addrinfo *resolved = 0;

    if (original < 0) {
        printf("UDP socket: errno=%d\n", errno);
        return 1;
    }
    fd = dup(original);
    close(original);
    if (fd < 0) {
        printf("UDP dup: errno=%d\n", errno);
        return 2;
    }
    sent = sendto(fd, dns_query, sizeof(dns_query), 0,
                  (const struct sockaddr *)&server, sizeof(server));
    if (sent != (ssize_t)sizeof(dns_query)) {
        printf("UDP sendto: sent=%ld errno=%d\n", (long)sent, errno);
        return 3;
    }
    received = recvfrom(fd, response, sizeof(response), 0,
                        (struct sockaddr *)&source, &source_length);
    if (!response_is_dns(response, received)) {
        printf("UDP recvfrom: length=%ld errno=%d\n", (long)received, errno);
        return 4;
    }
    child = fork();
    if (child < 0) {
        printf("UDP fork: errno=%d\n", errno);
        return 5;
    }
    if (child == 0) {
        close(fd);
        _exit(0);
    }
    if (waitpid(child, &child_status, 0) != child ||
        !WIFEXITED(child_status) || WEXITSTATUS(child_status) != 0) {
        printf("UDP fork/waitpid: status=%d errno=%d\n", child_status, errno);
        return 6;
    }
    sent = sendto(fd, dns_query, sizeof(dns_query), 0,
                  (const struct sockaddr *)&server, sizeof(server));
    received = sent == (ssize_t)sizeof(dns_query)
        ? recvfrom(fd, response, sizeof(response), 0,
                   (struct sockaddr *)&source, &source_length)
        : -1;
    if (!response_is_dns(response, received)) {
        printf("UDP parent fd after fork: length=%ld errno=%d\n", (long)received, errno);
        return 7;
    }
    puts("UDP socket fd remains live after child close");
    close(fd);
    printf("UDP sendto/recvfrom: DNS reply from %u.%u.%u.%u:%u (%ld bytes)\n",
           source.sin_addr.s_addr & 0xff,
           (source.sin_addr.s_addr >> 8) & 0xff,
           (source.sin_addr.s_addr >> 16) & 0xff,
           (source.sin_addr.s_addr >> 24) & 0xff,
           ntohs(source.sin_port), (long)received);

    fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (fd < 0 || connect(fd, (const struct sockaddr *)&server, sizeof(server)) != 0) {
        printf("UDP connect: errno=%d\n", errno);
        return 8;
    }
    if (write(fd, dns_query, sizeof(dns_query)) != (ssize_t)sizeof(dns_query) ||
        (received = read(fd, response, sizeof(response))) < 0 ||
        !response_is_dns(response, received)) {
        printf("connected UDP read/write: length=%ld errno=%d\n", (long)received, errno);
        return 9;
    }
    close(fd);
    puts("POSIX UDP socket, dup, bind/connect, read/write, and close passed");

    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_DGRAM;
    if (getaddrinfo("example.com", "53", &hints, &resolved) != 0) {
        puts("getaddrinfo failed");
        return 10;
    }
    {
        const struct sockaddr_in *resolved_address =
            (const struct sockaddr_in *)resolved->ai_addr;
        const unsigned char *octets =
            (const unsigned char *)&resolved_address->sin_addr.s_addr;
        printf("getaddrinfo example.com: %u.%u.%u.%u\n",
               octets[0], octets[1], octets[2], octets[3]);
    }
    freeaddrinfo(resolved);
    return 0;
}
