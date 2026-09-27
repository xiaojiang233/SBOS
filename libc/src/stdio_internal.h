#ifndef _SBOS_STDIO_INTERNAL_H
#define _SBOS_STDIO_INTERNAL_H
#include <stdio.h>
struct __sbos_FILE {
    int fd;
    int pushback;
    unsigned char eof;
    unsigned char error;
    unsigned char owned;
    unsigned char readable;
    unsigned char writable;
    unsigned char last_was_read;
    unsigned char allocated;
};
#endif
