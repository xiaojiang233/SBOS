#include <errno.h>
#include <sys/utime.h>

/* File timestamp mutation is not yet part of the SBOS native ABI. */
int utime(const char *path, const struct utimbuf *times) {
    (void)path;
    (void)times;
    errno = ENOSYS;
    return -1;
}
