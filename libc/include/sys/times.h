#ifndef _SBOS_SYS_TIMES_H
#define _SBOS_SYS_TIMES_H
#include <sys/types.h>

#define CLK_TCK 100
#define CLOCKS_PER_SEC 100
struct tms {
    clock_t tms_utime;
    clock_t tms_stime;
    clock_t tms_cutime;
    clock_t tms_cstime;
};
clock_t times(struct tms *buffer);

#endif
