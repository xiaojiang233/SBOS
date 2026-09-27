#include <errno.h>
#include <internal/syscall.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

int clock_gettime(int clock_id, struct timespec *time) {
    int64_t result;
    if (time == 0) { errno = EFAULT; return -1; }
    if (clock_id != CLOCK_REALTIME && clock_id != CLOCK_MONOTONIC) {
        errno = EINVAL;
        return -1;
    }
    result = __sbos_posix_checked_result(__sbos_syscall6(
        SBOS_CLOCK_GETTIME, (uint64_t)(uint32_t)clock_id,
        (uint64_t)(uintptr_t)time, 0, 0, 0, 0));
    if (result < 0) return -1;
    return 0;
}

int nanosleep(const struct timespec *request, struct timespec *remaining) {
    int64_t result;
    if (request == 0) { errno = EFAULT; return -1; }
    if (request->tv_sec < 0 || request->tv_nsec < 0 || request->tv_nsec >= 1000000000L) {
        errno = EINVAL;
        return -1;
    }
    result = __sbos_posix_checked_result(__sbos_syscall6(
        SBOS_THREAD_SLEEP, (uint64_t)request->tv_sec,
        (uint64_t)request->tv_nsec, 0, 0, 0, 0));
    if (result < 0) return -1;
    if (remaining != 0) { remaining->tv_sec = 0; remaining->tv_nsec = 0; }
    return 0;
}

time_t time(time_t *result) {
    struct timespec current;
    if (clock_gettime(CLOCK_REALTIME, &current) < 0) return (time_t)-1;
    if (result != 0) *result = current.tv_sec;
    return current.tv_sec;
}

static struct tm broken_down_time;
static char utc_name[] = "UTC";
char *tzname[2] = {utc_name, utc_name};
long timezone = 0;
int daylight = 0;

void tzset(void) {
    tzname[0] = utc_name;
    tzname[1] = utc_name;
    timezone = 0;
    daylight = 0;
}

static int leap_year(int year) {
    return (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
}

static struct tm *break_down_time(const time_t *value, struct tm *result) {
    static const int month_days[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    int64_t seconds;
    int64_t days;
    int64_t remainder;
    int64_t shifted_days;
    int64_t era;
    unsigned day_of_era;
    unsigned year_of_era;
    unsigned day_of_year;
    unsigned month_part;
    unsigned month;
    unsigned day;
    int year;
    int index;
    if (value == 0 || result == 0) { errno = EFAULT; return 0; }
    seconds = *value;
    days = seconds / 86400;
    remainder = seconds % 86400;
    if (remainder < 0) { remainder += 86400; --days; }
    shifted_days = days + 719468;
    era = (shifted_days >= 0 ? shifted_days : shifted_days - 146096) / 146097;
    day_of_era = (unsigned)(shifted_days - era * 146097);
    year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 -
                   day_of_era / 146096) / 365;
    year = (int)year_of_era + (int)(era * 400);
    day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    month_part = (5 * day_of_year + 2) / 153;
    day = day_of_year - (153 * month_part + 2) / 5 + 1;
    month = (unsigned)((int)month_part + (month_part < 10 ? 3 : -9));
    year += month <= 2;
    result->tm_sec = (int)(remainder % 60);
    result->tm_min = (int)((remainder / 60) % 60);
    result->tm_hour = (int)(remainder / 3600);
    result->tm_mday = (int)day;
    result->tm_mon = (int)month - 1;
    result->tm_year = year - 1900;
    result->tm_wday = (int)((days + 4) % 7);
    if (result->tm_wday < 0) result->tm_wday += 7;
    result->tm_yday = 0;
    for (index = 0; index < result->tm_mon; ++index)
        result->tm_yday += month_days[index] + (index == 1 && leap_year(year));
    result->tm_yday += result->tm_mday - 1;
    result->tm_isdst = 0;
    result->tm_gmtoff = 0;
    result->tm_zone = utc_name;
    return result;
}

struct tm *gmtime(const time_t *value) { return break_down_time(value, &broken_down_time); }
struct tm *localtime(const time_t *value) { return break_down_time(value, &broken_down_time); }
struct tm *gmtime_r(const time_t *value, struct tm *result) { return break_down_time(value, result); }
struct tm *localtime_r(const time_t *value, struct tm *result) { return break_down_time(value, result); }

static int append(char *buffer, size_t size, size_t *used,
                  const char *text, size_t length) {
    if (*used >= size || length >= size - *used) return 0;
    memcpy(buffer + *used, text, length);
    *used += length;
    buffer[*used] = '\0';
    return 1;
}

static int append_number(char *buffer, size_t size, size_t *used,
                         int value, const char *format) {
    char temporary[32];
    int length = snprintf(temporary, sizeof(temporary), format, value);
    return length >= 0 && append(buffer, size, used, temporary, (size_t)length);
}

size_t strftime(char *buffer, size_t size, const char *format,
                const struct tm *value) {
    static const char *const weekdays[] = {"Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"};
    static const char *const weekday_names[] = {"Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"};
    static const char *const months[] = {"Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
    static const char *const month_names[] = {"January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"};
    size_t used = 0;
    if (buffer == 0 || format == 0 || value == 0) { errno = EFAULT; return 0; }
    if (size == 0) return 0;
    buffer[0] = '\0';
    while (*format != '\0') {
        const char *text = 0;
        char conversion = *format++;
        if (conversion != '%') {
            if (!append(buffer, size, &used, &conversion, 1)) return 0;
            continue;
        }
        conversion = *format++;
        if (conversion == '\0') { errno = EINVAL; return 0; }
        switch (conversion) {
        case '%': text = "%"; break;
        case 'a': text = weekdays[(value->tm_wday + 7) % 7]; break;
        case 'A': text = weekday_names[(value->tm_wday + 7) % 7]; break;
        case 'b': case 'h': text = months[(value->tm_mon + 12) % 12]; break;
        case 'B': text = month_names[(value->tm_mon + 12) % 12]; break;
        case 'p': text = value->tm_hour < 12 ? "AM" : "PM"; break;
        case 'Z': text = value->tm_zone != 0 ? value->tm_zone : "UTC"; break;
        case 'n': text = "\n"; break;
        case 't': text = "\t"; break;
        default: break;
        }
        if (text != 0) {
            if (!append(buffer, size, &used, text, strlen(text))) return 0;
            continue;
        }
        if (conversion == 'c' || conversion == 'x' || conversion == 'X' ||
            conversion == 'D' || conversion == 'r' || conversion == 'R' ||
            conversion == 'T') {
            const char *nested = conversion == 'c' ? "%a %b %e %H:%M:%S %Y" :
                                 conversion == 'x' ? "%m/%d/%y" :
                                 conversion == 'X' ? "%H:%M:%S" :
                                 conversion == 'D' ? "%m/%d/%y" :
                                 conversion == 'r' ? "%I:%M:%S %p" :
                                 conversion == 'R' ? "%H:%M" : "%H:%M:%S";
            size_t length = strftime(buffer + used, size - used, nested, value);
            if (length == 0) return 0;
            used += length;
            continue;
        }
        switch (conversion) {
        case 'C': if (!append_number(buffer, size, &used, (value->tm_year + 1900) / 100, "%02d")) return 0; break;
        case 'd': if (!append_number(buffer, size, &used, value->tm_mday, "%02d")) return 0; break;
        case 'e': if (!append_number(buffer, size, &used, value->tm_mday, "%2d")) return 0; break;
        case 'F':
            if (!append_number(buffer, size, &used, value->tm_year + 1900, "%04d") ||
                !append(buffer, size, &used, "-", 1) ||
                !append_number(buffer, size, &used, value->tm_mon + 1, "%02d") ||
                !append(buffer, size, &used, "-", 1) ||
                !append_number(buffer, size, &used, value->tm_mday, "%02d")) return 0;
            break;
        case 'H': if (!append_number(buffer, size, &used, value->tm_hour, "%02d")) return 0; break;
        case 'I': if (!append_number(buffer, size, &used, (value->tm_hour + 11) % 12 + 1, "%02d")) return 0; break;
        case 'j': if (!append_number(buffer, size, &used, value->tm_yday + 1, "%03d")) return 0; break;
        case 'm': if (!append_number(buffer, size, &used, value->tm_mon + 1, "%02d")) return 0; break;
        case 'M': if (!append_number(buffer, size, &used, value->tm_min, "%02d")) return 0; break;
        case 'S': if (!append_number(buffer, size, &used, value->tm_sec, "%02d")) return 0; break;
        case 'u': if (!append_number(buffer, size, &used, value->tm_wday == 0 ? 7 : value->tm_wday, "%d")) return 0; break;
        case 'w': if (!append_number(buffer, size, &used, value->tm_wday, "%d")) return 0; break;
        case 'y': if (!append_number(buffer, size, &used, (value->tm_year + 1900) % 100, "%02d")) return 0; break;
        case 'Y': if (!append_number(buffer, size, &used, value->tm_year + 1900, "%04d")) return 0; break;
        case 'z': if (!append(buffer, size, &used, "+0000", 5)) return 0; break;
        default: errno = EINVAL; return 0;
        }
    }
    return used;
}

time_t mktime(struct tm *value) {
    (void)value;
    errno = ENOSYS;
    return (time_t)-1;
}
