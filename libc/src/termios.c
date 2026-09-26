#include <errno.h>
#include <internal/syscall.h>
#include <stdarg.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>

int tcgetattr(int fd, struct termios *attributes) {
    if (attributes == 0) { errno = EFAULT; return -1; }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_TTY_GET, (uint64_t)(uint32_t)fd,
        (uint64_t)(uintptr_t)attributes, 0, 0, 0, 0));
}

int tcsetattr(int fd, int action, const struct termios *attributes) {
    if (attributes == 0) { errno = EFAULT; return -1; }
    if (action != TCSANOW && action != TCSADRAIN && action != TCSAFLUSH) {
        errno = EINVAL;
        return -1;
    }
    return (int)__sbos_posix_checked_result(__sbos_syscall6(
        SBOS_POSIX_TTY_SET, (uint64_t)(uint32_t)fd,
        (uint64_t)(uint32_t)action, (uint64_t)(uintptr_t)attributes,
        0, 0, 0));
}

speed_t cfgetispeed(const struct termios *attributes) {
    if (attributes == 0) { errno = EFAULT; return 0; }
    return attributes->c_ispeed;
}
speed_t cfgetospeed(const struct termios *attributes) {
    if (attributes == 0) { errno = EFAULT; return 0; }
    return attributes->c_ospeed;
}
int cfsetispeed(struct termios *attributes, speed_t speed) {
    if (attributes == 0) { errno = EFAULT; return -1; }
    attributes->c_ispeed = speed;
    return 0;
}
int cfsetospeed(struct termios *attributes, speed_t speed) {
    if (attributes == 0) { errno = EFAULT; return -1; }
    attributes->c_ospeed = speed;
    return 0;
}
int tcflush(int fd, int queue) {
    if (queue < TCIFLUSH || queue > TCIOFLUSH) { errno = EINVAL; return -1; }
    struct termios attributes;
    if (tcgetattr(fd, &attributes) < 0) return -1;
    return tcsetattr(fd, TCSAFLUSH, &attributes);
}
int tcdrain(int fd) {
    if (!isatty(fd)) { if (errno == 0) errno = ENOTTY; return -1; }
    return 0;
}

int ioctl(int fd, unsigned long request, ...) {
    va_list arguments;
    void *argument;
    int64_t result;
    va_start(arguments, request);
    argument = va_arg(arguments, void *);
    va_end(arguments);
    result = __sbos_syscall6(SBOS_POSIX_IOCTL, (uint64_t)(uint32_t)fd,
                             request, (uint64_t)(uintptr_t)argument,
                             0, 0, 0);
    return (int)__sbos_posix_checked_result(result);
}
