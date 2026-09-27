#ifndef _SBOS_WCHAR_H
#define _SBOS_WCHAR_H
#include <stddef.h>

typedef unsigned int wint_t;
typedef unsigned int wctype_t;
typedef unsigned int mbstate_t;

size_t mbrtowc(wchar_t *wide_character, const char *bytes, size_t length,
               mbstate_t *state);
size_t wcrtomb(char *bytes, wchar_t wide_character, mbstate_t *state);
wchar_t *wmemcpy(wchar_t *destination, const wchar_t *source, size_t count);
wint_t towlower(wint_t wide_character);
wctype_t wctype(const char *name);
int iswctype(wint_t wide_character, wctype_t character_class);
size_t wcslen(const wchar_t *string);
wchar_t *wcscat(wchar_t *destination, const wchar_t *source);

#endif
