#ifndef _SBOS_ASSERT_H
#define _SBOS_ASSERT_H

void abort(void);

#ifdef NDEBUG
#define assert(expression) ((void)0)
#else
#define assert(expression) ((expression) ? (void)0 : abort())
#endif

#endif
