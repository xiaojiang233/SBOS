#include <errno.h>
#include <signal.h>

sighandler_t signal(int signal_number, sighandler_t handler) {
    (void)signal_number;
    (void)handler;
    errno = ENOSYS;
    return SIG_ERR;
}

int raise(int signal_number) {
    (void)signal_number;
    errno = ENOSYS;
    return -1;
}

static int signal_bit(int signal_number, uint64_t *bit) {
    if (signal_number < 1 || signal_number > 64) { errno = EINVAL; return -1; }
    *bit = UINT64_C(1) << (signal_number - 1);
    return 0;
}

int sigemptyset(sigset_t *set) {
    if (set == 0) { errno = EFAULT; return -1; }
    *set = 0;
    return 0;
}

int sigfillset(sigset_t *set) {
    if (set == 0) { errno = EFAULT; return -1; }
    *set = UINT64_MAX;
    return 0;
}

int sigaddset(sigset_t *set, int signal_number) {
    uint64_t bit;
    if (set == 0) { errno = EFAULT; return -1; }
    if (signal_bit(signal_number, &bit) < 0) return -1;
    *set |= bit;
    return 0;
}

int sigdelset(sigset_t *set, int signal_number) {
    uint64_t bit;
    if (set == 0) { errno = EFAULT; return -1; }
    if (signal_bit(signal_number, &bit) < 0) return -1;
    *set &= ~bit;
    return 0;
}

int sigismember(const sigset_t *set, int signal_number) {
    uint64_t bit;
    if (set == 0) { errno = EFAULT; return -1; }
    if (signal_bit(signal_number, &bit) < 0) return -1;
    return (*set & bit) != 0;
}

int sigprocmask(int how, const sigset_t *set, sigset_t *old_set) {
    (void)how; (void)set; (void)old_set;
    errno = ENOSYS;
    return -1;
}

int sigaction(int signal_number, const struct sigaction *action,
              struct sigaction *old_action) {
    (void)signal_number; (void)action; (void)old_action;
    errno = ENOSYS;
    return -1;
}

int kill(pid_t process, int signal_number) {
    (void)process; (void)signal_number;
    errno = ENOSYS;
    return -1;
}
