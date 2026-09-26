#ifndef _SBOS_SYS_TYPES_H
#define _SBOS_SYS_TYPES_H
#include <stddef.h>
#include <stdint.h>
typedef int64_t ssize_t;
typedef int32_t pid_t;
typedef int64_t off_t;
typedef int64_t clock_t;
typedef int64_t time_t;
typedef uint32_t mode_t;
typedef uint32_t uid_t;
typedef uint32_t gid_t;
typedef uint64_t dev_t;
typedef uint64_t ino_t;
typedef uint64_t nlink_t;
typedef int64_t blksize_t;
typedef int64_t blkcnt_t;
typedef int64_t suseconds_t;
typedef uint32_t useconds_t;
typedef unsigned int u_int;
typedef unsigned long u_long;
typedef uint32_t speed_t;
typedef int16_t bits16_t;
typedef uint16_t u_bits16_t;
typedef int32_t bits32_t;
typedef uint32_t u_bits32_t;
typedef int64_t bits64_t;
typedef uint64_t u_bits64_t;
#endif
