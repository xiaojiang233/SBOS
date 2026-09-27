#ifndef _SBOS_STDIO_EXT_H
#define _SBOS_STDIO_EXT_H

#include <stdio.h>

/* GNU stdio extensions supported by the current unbuffered SBOS streams. */
size_t __fpending(FILE *stream);
void __fpurge(FILE *stream);
size_t __freadahead(FILE *stream);
int __freading(FILE *stream);
int __fwriting(FILE *stream);
void __fseterr(FILE *stream);

#endif
