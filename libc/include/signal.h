#ifndef _SBOS_SIGNAL_H
#define _SBOS_SIGNAL_H
#include <stdint.h>
#include <sys/types.h>
typedef int sig_atomic_t;
typedef uint64_t sigset_t;
typedef void (*sighandler_t)(int);
#define SIG_DFL ((sighandler_t)0)
#define SIG_IGN ((sighandler_t)1)
#define SIG_ERR ((sighandler_t)-1)
#define SIGHUP 1
#define SIGINT 2
#define SIGQUIT 3
#define SIGILL 4
#define SIGTRAP 5
#define SIGABRT 6
#define SIGBUS 7
#define SIGFPE 8
#define SIGKILL 9
#define SIGUSR1 10
#define SIGSEGV 11
#define SIGUSR2 12
#define SIGPIPE 13
#define SIGALRM 14
#define SIGTERM 15
#define SIGCHLD 17
#define SIGCONT 18
#define SIGSTOP 19
#define SIGTSTP 20
#define SIGTTIN 21
#define SIGTTOU 22
#define SIGWINCH 28
#define SIG_BLOCK 0
#define SIG_UNBLOCK 1
#define SIG_SETMASK 2
struct sigaction {
    sighandler_t sa_handler;
    sigset_t sa_mask;
    int sa_flags;
};
sighandler_t signal(int signal_number, sighandler_t handler);
int raise(int signal_number);
int sigemptyset(sigset_t *set);
int sigfillset(sigset_t *set);
int sigaddset(sigset_t *set, int signal_number);
int sigdelset(sigset_t *set, int signal_number);
int sigismember(const sigset_t *set, int signal_number);
int sigprocmask(int how, const sigset_t *set, sigset_t *old_set);
int sigaction(int signal_number, const struct sigaction *action,
              struct sigaction *old_action);
int kill(pid_t process, int signal_number);
#endif
