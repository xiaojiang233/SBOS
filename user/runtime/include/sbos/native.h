#ifndef SBOS_NATIVE_H
#define SBOS_NATIVE_H

#include <stddef.h>
#include <stdint.h>

typedef uint32_t sbos_handle_t;

enum sbos_syscall {
    SBOS_HANDLE_CLOSE = 0,
    SBOS_FILE_OPEN = 1,
    SBOS_FILE_READ = 2,
    SBOS_FILE_WRITE = 3,
    SBOS_PROCESS_EXIT = 4,
    SBOS_PROCESS_SPAWN = 5,
    SBOS_THREAD_CREATE = 6,
    SBOS_HANDLE_WAIT = 7,
    SBOS_CHANNEL_CREATE = 8,
    SBOS_CHANNEL_SEND = 9,
    SBOS_CHANNEL_RECEIVE = 10,
    SBOS_MEMORY_MAP = 11,
    SBOS_MEMORY_UNMAP = 12,
    SBOS_CONSOLE_READ = 13,
    SBOS_CONSOLE_WRITE = 14,
    SBOS_DIRECTORY_OPEN = 15,
    SBOS_DIRECTORY_READ = 16,
    SBOS_DIRECTORY_CHANGE = 17,
    SBOS_DIRECTORY_CURRENT = 18,
    SBOS_SYSTEM_QUERY = 19,
    SBOS_CONFIG_SET = 20,
    SBOS_IO_SUBMIT = 21,
    SBOS_CONFIG_DELETE = 22,
    SBOS_CONFIG_WATCH = 23,
    SBOS_CONFIG_BEGIN = 24,
    SBOS_CONFIG_TRANSACTION_SET = 25,
    SBOS_CONFIG_TRANSACTION_DELETE = 26,
    SBOS_CONFIG_COMMIT = 27,
    SBOS_EVENT_RESET = 28,
    SBOS_SERVICE_LOOKUP = 29,
    SBOS_TTY_SET_FOREGROUND = 30,
    SBOS_PROCESS_ID = 31,
    SBOS_SCHEDULER_TICKS = 32,
    SBOS_STAT_PATH = 33,
    SBOS_HANDLE_STAT = 34,
    SBOS_HANDLE_DUPLICATE = 35,
    SBOS_PROCESS_UID = 36,
    SBOS_PROCESS_GID = 37,
    SBOS_FILE_SEEK = 38,
    SBOS_PROCESS_CPU_TICKS = 39,
    SBOS_PROCESS_WAITPID = 40,
    SBOS_PROCESS_PARENT_ID = 41,
    SBOS_PIPE_CREATE = 42,
    SBOS_PIPE_READ = 43,
    SBOS_PIPE_WRITE = 44,
    SBOS_PROCESS_FORK = 45,
    SBOS_POSIX_OPEN = 46,
    SBOS_POSIX_READ = 47,
    SBOS_POSIX_WRITE = 48,
    SBOS_POSIX_CLOSE = 49,
    SBOS_POSIX_DUP = 50,
    SBOS_POSIX_DUP2 = 51,
    SBOS_POSIX_PIPE = 52,
    SBOS_POSIX_FSTAT = 53,
    SBOS_POSIX_LSEEK = 54,
    SBOS_POSIX_FCNTL = 55,
    SBOS_POSIX_ISATTY = 56,
    SBOS_POSIX_UNLINK = 57,
    SBOS_POSIX_TTY_GET = 58,
    SBOS_POSIX_TTY_SET = 59,
    SBOS_POSIX_IOCTL = 60,
    SBOS_POSIX_EXECVE = 61,
    SBOS_POSIX_UMASK = 62,
    SBOS_POSIX_RENAME = 63
};

static inline int64_t sbos_call6(uint64_t number, uint64_t a0, uint64_t a1,
                                 uint64_t a2, uint64_t a3, uint64_t a4,
                                 uint64_t a5) {
    register uint64_t r10 __asm__("r10") = a3;
    register uint64_t r8 __asm__("r8") = a4;
    register uint64_t r9 __asm__("r9") = a5;
    __asm__ volatile("int $0x80"
                     : "+a"(number)
                     : "D"(a0), "S"(a1), "d"(a2), "r"(r10), "r"(r8), "r"(r9)
                     : "rcx", "r11", "memory", "cc");
    return (int64_t)number;
}

#endif
