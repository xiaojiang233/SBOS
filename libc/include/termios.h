#ifndef _SBOS_TERMIOS_H
#define _SBOS_TERMIOS_H
#include <sys/types.h>

typedef uint32_t tcflag_t;
typedef uint8_t cc_t;
#define NCCS 32

struct termios {
    tcflag_t c_iflag;
    tcflag_t c_oflag;
    tcflag_t c_cflag;
    tcflag_t c_lflag;
    cc_t c_line;
    cc_t c_cc[NCCS];
    speed_t c_ispeed;
    speed_t c_ospeed;
};

#define TCSANOW 0
#define TCSADRAIN 1
#define TCSAFLUSH 2
#define TCIFLUSH 0
#define TCOFLUSH 1
#define TCIOFLUSH 2
#define ISIG 0x0001
#define ICANON 0x0002
#define ECHO 0x0008
#define ECHOE 0x0010
#define ECHOK 0x0020
#define ECHONL 0x0040
#define IEXTEN 0x8000
#define IXON 0x0400
#define ICRNL 0x0100
#define INLCR 0x0040
#define ISTRIP 0x0020
#define INPCK 0x0010
#define OPOST 0x0001
#define ONLCR 0x0004
#define OCRNL 0x0008
#define ONOCR 0x0010
#define ONLRET 0x0020
#define CS8 0x0030
#define CSIZE 0x0030
#define PARENB 0x0100
#define VINTR 0
#define VQUIT 1
#define VERASE 2
#define VKILL 3
#define VEOF 4
#define VTIME 5
#define VMIN 6
#define VSTART 8
#define VSTOP 9
#define VSUSP 10
#define VEOL 11
#define VREPRINT 12
#define VDISCARD 13
#define VWERASE 14
#define VLNEXT 15
#define VEOL2 16
#define TCOOFF 0
#define TCOON 1
#define TCIOFF 2
#define TCION 3

int tcgetattr(int fd, struct termios *attributes);
int tcsetattr(int fd, int action, const struct termios *attributes);
speed_t cfgetispeed(const struct termios *attributes);
speed_t cfgetospeed(const struct termios *attributes);
int cfsetispeed(struct termios *attributes, speed_t speed);
int cfsetospeed(struct termios *attributes, speed_t speed);
int tcflush(int fd, int queue);
int tcdrain(int fd);
int tcflow(int fd, int action);

#endif
