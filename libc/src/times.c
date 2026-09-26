#include <errno.h>
#include <internal/syscall.h>
#include <stdint.h>
#include <sys/times.h>

clock_t times(struct tms *buffer) {
    int64_t user_ticks;
    int64_t elapsed_ticks;
    if (buffer == 0) { errno = EFAULT; return (clock_t)-1; }
    user_ticks = __sbos_checked_result(__sbos_syscall6(
        SBOS_PROCESS_CPU_TICKS, 0, 0, 0, 0, 0, 0));
    if (user_ticks < 0) return (clock_t)-1;
    elapsed_ticks = __sbos_checked_result(__sbos_syscall6(
        SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0));
    if (elapsed_ticks < 0) return (clock_t)-1;
    buffer->tms_utime = (clock_t)user_ticks;
    buffer->tms_stime = 0;
    buffer->tms_cutime = 0;
    buffer->tms_cstime = 0;
    return (clock_t)elapsed_ticks;
}
