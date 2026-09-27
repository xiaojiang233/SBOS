#ifndef _SBOS_TIME_H
#define _SBOS_TIME_H
#include <stdint.h>
#include <sys/types.h>
struct timespec { time_t tv_sec; long tv_nsec; };
struct tm {
    int tm_sec;
    int tm_min;
    int tm_hour;
    int tm_mday;
    int tm_mon;
    int tm_year;
    int tm_wday;
    int tm_yday;
    int tm_isdst;
    long tm_gmtoff;
    char *tm_zone;
};
#define CLOCK_REALTIME 0
#define CLOCK_MONOTONIC 1
int clock_gettime(int clock_id, struct timespec *time);
int nanosleep(const struct timespec *request, struct timespec *remaining);
time_t time(time_t *result);
extern char *tzname[2];
extern long timezone;
extern int daylight;
void tzset(void);
struct tm *gmtime(const time_t *time_value);
struct tm *localtime(const time_t *time_value);
struct tm *gmtime_r(const time_t *time_value, struct tm *result);
struct tm *localtime_r(const time_t *time_value, struct tm *result);
size_t strftime(char *buffer, size_t size, const char *format,
                const struct tm *time_value);
time_t mktime(struct tm *time_value);
#endif
