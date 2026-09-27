# Network stack bring-up

## Implemented path

```text
QEMU user-mode network
  → Intel 82540EM (e1000) PCI function
  → SBOS PCI scanner and MMIO mapping
  → e1000 DMA RX/TX rings backed by PMM pages
  → smoltcp Ethernet interface
  → ARP / IPv4 / ICMP / UDP / TCP / DHCPv4 / DNS protocol modules
```

The e1000 implementation is scoped to QEMU's `pc` machine and device ID
`8086:100e`. It uses 32-entry RX/TX rings, 2 KiB packet buffers, polling, and
DMA memory below the kernel's current 1 GiB physical direct-map limit. It is
not a general PCI or production e1000 driver yet.

The interface starts with `10.0.2.15/24`, gateway `10.0.2.2`, and DNS
`10.0.2.3` as boot-time defaults. DHCPv4 then obtains and applies a lease,
router, and DNS server list. A QEMU run confirmed a DHCP lease event and
continued into the Ring 3 shell. Stack polling currently runs from the 100 Hz
PIT interrupt; a dedicated network worker and interrupt-driven RX queue are
future work.

## Ring 3 UDP support

- The POSIX layer implements IPv4 `SOCK_DGRAM`: `socket`, `bind`, `connect`,
  `sendto`, `recvfrom`, `send`, and `recv`. Socket IDs are stored in process
  descriptors; `dup`, `fork`, close-on-exec, process exit, and `close` update
  the underlying socket reference count.
- `network-probe` ran from Bash in QEMU. It duplicated a UDP fd, closed the
  original, sent a DNS A query to `10.0.2.3:53`, received the response and
  source address, then tested connected UDP with fd `write`/`read`.
- `getaddrinfo` now queries the DHCP-provided resolver over that UDP API.
  QEMU resolved `example.com` to `172.66.147.243` during the smoke run. The
  initial implementation supports IPv4 A records and a small service-name
  table; reverse lookup, search domains, AAAA, and DNS-over-TCP fallback are
  not implemented.

## Not implemented yet

- TCP active `connect` is implemented but still needs an outbound runtime test;
  backlog is limited to one and stream options/shutdown semantics are sparse.
- Legacy `gethostbyname`/hostent interfaces and `getnameinfo` reverse lookup.
- Route manager, IPv6, loopback, and multiple interfaces.
- Interrupt-driven NIC service, non-QEMU hardware coverage, and PCI ECAM/ACPI
  enumeration.
- GNU network utilities. UDP and the TCP listener/stream path are available,
  but authentication, broader TCP semantics, and common client compatibility
  remain incomplete.

The protocol engine is upstream no_std smoltcp 0.14.0 (0BSD). It provides
protocol state machines; SBOS retains ownership of the hardware driver, syscall
ABI, socket descriptors, and user-facing policy.
