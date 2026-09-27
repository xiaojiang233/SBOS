#include <errno.h>
#include <sys/time.h>
#include <time.h>

int gettimeofday(struct timeval *value, void *zone) {
    struct timespec current;
    (void)zone;
    if (value == 0) { errno = EFAULT; return -1; }
    if (clock_gettime(CLOCK_REALTIME, &current) < 0) return -1;
    value->tv_sec = current.tv_sec;
    value->tv_usec = (suseconds_t)(current.tv_nsec / 1000);
    return 0;
}
