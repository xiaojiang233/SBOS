#ifndef _SBOS_ASSERT_H
#define _SBOS_ASSERT_H
#include <stdlib.h>
#endif

#ifdef assert
#undef assert
#endif
#ifndef NDEBUG
#define assert(expression) ((expression) ? (void)0 : abort())
#else
#define assert(expression) ((void)0)
#endif
