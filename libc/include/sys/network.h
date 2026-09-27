#ifndef _SBOS_SYS_NETWORK_H
#define _SBOS_SYS_NETWORK_H

#include <stdint.h>

/* Versioned kernel network configuration returned by the Native syscall. */
struct sbos_network_config {
    uint32_t version;
    uint8_t ipv4[4];
    uint8_t gateway[4];
    uint32_t dns_count;
    uint8_t dns[2][4];
};

#endif
