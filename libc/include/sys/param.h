#ifndef _SBOS_SYS_PARAM_H
#define _SBOS_SYS_PARAM_H
#include <limits.h>
#define MAXPATHLEN 512
#define MAXHOSTNAMELEN 64
#define NBBY 8
#define NOFILE 128
#ifndef PATH_MAX
#define PATH_MAX MAXPATHLEN
#endif
#ifndef NAME_MAX
#define NAME_MAX 64
#endif
#endif
