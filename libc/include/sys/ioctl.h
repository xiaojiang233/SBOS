#ifndef _SBOS_SYS_IOCTL_H
#define _SBOS_SYS_IOCTL_H

#include <stdint.h>

struct winsize {
    uint16_t ws_row;
    uint16_t ws_col;
    uint16_t ws_xpixel;
    uint16_t ws_ypixel;
};

#define TIOCGWINSZ 0x5413UL
#define TIOCSWINSZ 0x5414UL

int ioctl(int fd, unsigned long request, ...);

#endif
