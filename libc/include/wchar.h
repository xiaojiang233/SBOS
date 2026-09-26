#ifndef _SBOS_WCHAR_H
#define _SBOS_WCHAR_H
#include <stddef.h>

typedef unsigned int wint_t;
typedef unsigned int wctype_t;
typedef unsigned int mbstate_t;

size_t mbrtowc(wchar_t *wide_character, const char *bytes, size_t length,
               mbstate_t *state);
wint_t towlower(wint_t wide_character);

#endif
