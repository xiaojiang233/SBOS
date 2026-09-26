#include <errno.h>
#include <sys/time.h>

/* The UEFI boot protocol does not yet pass a real-time clock value. */
int gettimeofday(struct timeval *value, void *zone) {
    (void)value;
    (void)zone;
    errno = ENOSYS;
    return -1;
}
