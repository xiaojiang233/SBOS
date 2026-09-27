#include <errno.h>
#include <internal/syscall.h>
#include <netdb.h>
#include <netinet/in.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/network.h>
#include <time.h>
#include <unistd.h>

#define DNS_PORT 53
#define DNS_PACKET_CAPACITY 512
#define DNS_QUERY_ATTEMPTS 160
#define DNS_RETRY_NANOSECONDS 10000000L

static uint16_t next_dns_id = 0x5342;

static uint16_t read_u16(const unsigned char *bytes) {
    return (uint16_t)(((uint16_t)bytes[0] << 8) | bytes[1]);
}

static void write_u16(unsigned char *bytes, uint16_t value) {
    bytes[0] = (unsigned char)(value >> 8);
    bytes[1] = (unsigned char)value;
}

static int parse_ipv4(const char *text, unsigned char address[4]) {
    unsigned part;
    int component;
    const char *cursor = text;
    if (text == 0 || *text == '\0') return 0;
    for (component = 0; component < 4; ++component) {
        part = 0;
        if (*cursor < '0' || *cursor > '9') return 0;
        do {
            part = part * 10u + (unsigned)(*cursor - '0');
            if (part > 255) return 0;
            ++cursor;
        } while (*cursor >= '0' && *cursor <= '9');
        address[component] = (unsigned char)part;
        if (component != 3) {
            if (*cursor++ != '.') return 0;
        } else if (*cursor != '\0') {
            return 0;
        }
    }
    return 1;
}

static int encode_dns_name(const char *name, unsigned char *out, size_t capacity,
                           size_t *written) {
    size_t length;
    size_t used = 0;
    size_t label_start = 0;
    size_t index;
    if (name == 0 || *name == '\0') return 0;
    length = strlen(name);
    if (length != 0 && name[length - 1] == '.') --length;
    if (length == 0 || length > 253) return 0;
    for (index = 0; index <= length; ++index) {
        if (index != length && name[index] != '.') {
            unsigned char character = (unsigned char)name[index];
            if (character <= 0x20 || character >= 0x7f) return 0;
            continue;
        }
        if (index == label_start || index - label_start > 63) return 0;
        if (used + 1 + (index - label_start) + 1 > capacity) return 0;
        out[used++] = (unsigned char)(index - label_start);
        memcpy(out + used, name + label_start, index - label_start);
        used += index - label_start;
        label_start = index + 1;
    }
    if (used >= capacity) return 0;
    out[used++] = 0;
    *written = used;
    return 1;
}

static int skip_dns_name(const unsigned char *packet, size_t length, size_t *offset) {
    size_t cursor = *offset;
    unsigned labels = 0;
    while (cursor < length && labels++ < 128) {
        unsigned char size = packet[cursor++];
        if (size == 0) {
            *offset = cursor;
            return 1;
        }
        if ((size & 0xc0) == 0xc0) {
            if (cursor >= length) return 0;
            *offset = cursor + 1;
            return 1;
        }
        if ((size & 0xc0) != 0 || size > 63 || cursor + size > length) return 0;
        cursor += size;
    }
    return 0;
}

static int parse_dns_response(const unsigned char *packet, size_t length,
                              uint16_t transaction, unsigned char address[4]) {
    uint16_t flags;
    uint16_t questions;
    uint16_t answers;
    size_t offset = 12;
    unsigned index;
    if (length < 12 || read_u16(packet) != transaction) return EAI_AGAIN;
    flags = read_u16(packet + 2);
    if ((flags & 0x8000) == 0 || (flags & 0x0200) != 0) return EAI_AGAIN;
    if ((flags & 0x000f) == 3) return EAI_NONAME;
    if ((flags & 0x000f) != 0) return EAI_FAIL;
    questions = read_u16(packet + 4);
    answers = read_u16(packet + 6);
    if (questions == 0) return EAI_FAIL;
    for (index = 0; index < questions; ++index) {
        if (!skip_dns_name(packet, length, &offset) || offset + 4 > length) return EAI_FAIL;
        offset += 4;
    }
    for (index = 0; index < answers; ++index) {
        uint16_t type;
        uint16_t class_code;
        uint16_t data_length;
        if (!skip_dns_name(packet, length, &offset) || offset + 10 > length) return EAI_FAIL;
        type = read_u16(packet + offset);
        class_code = read_u16(packet + offset + 2);
        data_length = read_u16(packet + offset + 8);
        offset += 10;
        if (offset + data_length > length) return EAI_FAIL;
        if (type == 1 && class_code == 1 && data_length == 4) {
            memcpy(address, packet + offset, 4);
            return 0;
        }
        offset += data_length;
    }
    return EAI_NONAME;
}

static int dns_lookup(const char *name, unsigned char address[4]) {
    struct sbos_network_config config;
    struct timespec retry = {0, DNS_RETRY_NANOSECONDS};
    unsigned char query[DNS_PACKET_CAPACITY];
    unsigned char response[DNS_PACKET_CAPACITY];
    size_t name_length;
    size_t query_length;
    uint16_t transaction = ++next_dns_id;
    uint32_t server_index;
    int64_t config_result;
    int socket_fd;
    int final_error = EAI_AGAIN;

    memset(&config, 0, sizeof(config));
    config_result = __sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_NETWORK_CONFIG, (uint64_t)(uintptr_t)&config,
        sizeof(config), 0, 0, 0, 0));
    if (config_result < 0 || config.version != 1 || config.dns_count == 0)
        return EAI_AGAIN;
    if (!encode_dns_name(name, query + 12, sizeof(query) - 16, &name_length))
        return EAI_NONAME;

    memset(query, 0, 12);
    write_u16(query, transaction);
    write_u16(query + 2, 0x0100); /* recursion desired */
    write_u16(query + 4, 1);
    query_length = 12 + name_length;
    write_u16(query + query_length, 1); /* A */
    write_u16(query + query_length + 2, 1); /* IN */
    query_length += 4;

    socket_fd = socket(AF_INET, SOCK_DGRAM | SOCK_NONBLOCK, IPPROTO_UDP);
    if (socket_fd < 0) return EAI_SYSTEM;
    if (query_length > sizeof(query)) {
        close(socket_fd);
        return EAI_FAIL;
    }
    for (server_index = 0; server_index < config.dns_count && server_index < 2; ++server_index) {
        struct sockaddr_in server;
        unsigned attempt;
        if (config.dns[server_index][0] == 0 && config.dns[server_index][1] == 0 &&
            config.dns[server_index][2] == 0 && config.dns[server_index][3] == 0)
            continue;
        memset(&server, 0, sizeof(server));
        server.sin_family = AF_INET;
        server.sin_port = htons(DNS_PORT);
        memcpy(&server.sin_addr.s_addr, config.dns[server_index], 4);
        if (sendto(socket_fd, query, query_length, 0,
                   (const struct sockaddr *)&server, sizeof(server)) < 0) {
            final_error = EAI_AGAIN;
            continue;
        }
        for (attempt = 0; attempt < DNS_QUERY_ATTEMPTS; ++attempt) {
            ssize_t received = recvfrom(socket_fd, response, sizeof(response), 0, 0, 0);
            if (received >= 0) {
                int result = parse_dns_response(response, (size_t)received, transaction, address);
                if (result == 0 || result == EAI_NONAME || result == EAI_FAIL) {
                    close(socket_fd);
                    return result;
                }
            } else if (errno != EAGAIN && errno != EWOULDBLOCK) {
                final_error = EAI_SYSTEM;
                break;
            }
            (void)nanosleep(&retry, 0);
        }
    }
    close(socket_fd);
    return final_error;
}

static int parse_service(const char *service, int flags, uint16_t *port) {
    const char *cursor;
    unsigned value = 0;
    if (service == 0) { *port = 0; return 0; }
    if (*service == '\0') return EAI_SERVICE;
    for (cursor = service; *cursor != '\0'; ++cursor) {
        if (*cursor < '0' || *cursor > '9') { value = 65536; break; }
        value = value * 10 + (unsigned)(*cursor - '0');
        if (value > 65535) return EAI_SERVICE;
    }
    if (value <= 65535) { *port = (uint16_t)value; return 0; }
    if (flags & AI_NUMERICSERV) return EAI_NONAME;
    if (strcmp(service, "domain") == 0 || strcmp(service, "dns") == 0) value = 53;
    else if (strcmp(service, "http") == 0) value = 80;
    else if (strcmp(service, "https") == 0) value = 443;
    else if (strcmp(service, "ssh") == 0) value = 22;
    else if (strcmp(service, "ftp") == 0) value = 21;
    else return EAI_SERVICE;
    *port = (uint16_t)value;
    return 0;
}

int getaddrinfo(const char *node, const char *service,
                const struct addrinfo *hints, struct addrinfo **result) {
    struct addrinfo defaults;
    struct addrinfo *entry;
    struct sockaddr_in *address;
    unsigned char ipv4[4];
    uint16_t port;
    int flags = 0;
    int family = AF_UNSPEC;
    int socket_type = 0;
    int protocol = 0;
    int error;
    if (result == 0) return EAI_FAIL;
    *result = 0;
    memset(&defaults, 0, sizeof(defaults));
    if (hints == 0) hints = &defaults;
    flags = hints->ai_flags;
    if (flags & ~(AI_PASSIVE | AI_CANONNAME | AI_NUMERICHOST | AI_NUMERICSERV | AI_ADDRCONFIG))
        return EAI_BADFLAGS;
    if (hints->ai_family != AF_UNSPEC && hints->ai_family != AF_INET) return EAI_FAMILY;
    family = AF_INET;
    socket_type = hints->ai_socktype;
    protocol = hints->ai_protocol;
    if (socket_type == 0) {
        socket_type = protocol == IPPROTO_TCP ? SOCK_STREAM : SOCK_DGRAM;
    }
    if (socket_type != 0 && socket_type != SOCK_DGRAM && socket_type != SOCK_STREAM)
        return EAI_SOCKTYPE;
    if ((protocol != 0 && protocol != IPPROTO_UDP && protocol != IPPROTO_TCP) ||
        (socket_type == SOCK_DGRAM && protocol == IPPROTO_TCP) ||
        (socket_type == SOCK_STREAM && protocol == IPPROTO_UDP))
        return EAI_SOCKTYPE;
    error = parse_service(service, flags, &port);
    if (error != 0) return error;

    if (node == 0) {
        ipv4[0] = (flags & AI_PASSIVE) ? 0 : 127;
        ipv4[1] = (flags & AI_PASSIVE) ? 0 : 0;
        ipv4[2] = (flags & AI_PASSIVE) ? 0 : 0;
        ipv4[3] = (flags & AI_PASSIVE) ? 0 : 1;
    } else if (!parse_ipv4(node, ipv4)) {
        if (flags & AI_NUMERICHOST) return EAI_NONAME;
        error = dns_lookup(node, ipv4);
        if (error != 0) return error;
    }

    entry = (struct addrinfo *)calloc(1, sizeof(*entry));
    address = (struct sockaddr_in *)calloc(1, sizeof(*address));
    if (entry == 0 || address == 0) {
        free(entry);
        free(address);
        return EAI_MEMORY;
    }
    address->sin_family = AF_INET;
    address->sin_port = htons(port);
    memcpy(&address->sin_addr.s_addr, ipv4, 4);
    entry->ai_flags = flags;
    entry->ai_family = family;
    entry->ai_socktype = socket_type != 0 ? socket_type : SOCK_DGRAM;
    entry->ai_protocol = protocol != 0 ? protocol :
        (entry->ai_socktype == SOCK_STREAM ? IPPROTO_TCP : IPPROTO_UDP);
    entry->ai_addrlen = sizeof(*address);
    entry->ai_addr = (struct sockaddr *)address;
    if ((flags & AI_CANONNAME) && node != 0) {
        entry->ai_canonname = strdup(node);
        if (entry->ai_canonname == 0) {
            free(address);
            free(entry);
            return EAI_MEMORY;
        }
    }
    *result = entry;
    return 0;
}

void freeaddrinfo(struct addrinfo *result) {
    while (result != 0) {
        struct addrinfo *next = result->ai_next;
        free(result->ai_addr);
        free(result->ai_canonname);
        free(result);
        result = next;
    }
}

const char *gai_strerror(int error) {
    switch (error) {
    case 0: return "success";
    case EAI_BADFLAGS: return "invalid address information flags";
    case EAI_NONAME: return "name or service not known";
    case EAI_AGAIN: return "temporary name resolution failure";
    case EAI_FAIL: return "non-recoverable name resolution failure";
    case EAI_FAMILY: return "address family not supported";
    case EAI_SOCKTYPE: return "socket type not supported for this service";
    case EAI_SERVICE: return "service not supported for socket type";
    case EAI_MEMORY: return "memory allocation failure";
    case EAI_SYSTEM: return "system error";
    default: return "unknown address resolution error";
    }
}

int getnameinfo(const struct sockaddr *address, socklen_t address_length,
                char *host, socklen_t host_length, char *service,
                socklen_t service_length, int flags) {
    const struct sockaddr_in *ipv4;
    int host_count;
    int service_count;
    if (address == 0 || address_length < sizeof(struct sockaddr_in)) return EAI_FAMILY;
    if (flags & ~(NI_NUMERICHOST | NI_NUMERICSERV)) return EAI_BADFLAGS;
    if (address->sa_family != AF_INET) return EAI_FAMILY;
    ipv4 = (const struct sockaddr_in *)address;
    if (host != 0) {
        if (!(flags & NI_NUMERICHOST)) return EAI_NONAME;
        host_count = snprintf(host, host_length, "%u.%u.%u.%u",
            ((const unsigned char *)&ipv4->sin_addr.s_addr)[0],
            ((const unsigned char *)&ipv4->sin_addr.s_addr)[1],
            ((const unsigned char *)&ipv4->sin_addr.s_addr)[2],
            ((const unsigned char *)&ipv4->sin_addr.s_addr)[3]);
        if (host_count < 0 || (socklen_t)host_count >= host_length) return EAI_FAIL;
    }
    if (service != 0) {
        if (!(flags & NI_NUMERICSERV)) return EAI_NONAME;
        service_count = snprintf(service, service_length, "%u", ntohs(ipv4->sin_port));
        if (service_count < 0 || (socklen_t)service_count >= service_length) return EAI_FAIL;
    }
    return 0;
}
