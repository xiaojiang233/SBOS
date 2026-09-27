#ifndef _SBOS_SYS_TIME_H
#define _SBOS_SYS_TIME_H
#include <sys/types.h>

#ifndef _SBOS_TIMEVAL_DEFINED
#define _SBOS_TIMEVAL_DEFINED
struct timeval {
    time_t tv_sec;
    suseconds_t tv_usec;
};
#endif

struct timezone {
    int tz_minuteswest;
    int tz_dsttime;
};

int gettimeofday(struct timeval *value, void *zone);

#endif
