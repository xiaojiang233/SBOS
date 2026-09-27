#include <errno.h>
#include <ctype.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

/* The current SBOS locale is the single-byte C locale. */
size_t mbrtowc(wchar_t *wide_character, const char *bytes, size_t length,
               mbstate_t *state) {
    unsigned char value;
    (void)state;
    if (bytes == 0) {
        if (wide_character != 0) *wide_character = 0;
        return 0;
    }
    if (length == 0) return (size_t)-2;
    value = (unsigned char)bytes[0];
    if (value > 0x7f) { errno = EILSEQ; return (size_t)-1; }
    if (wide_character != 0) *wide_character = (wchar_t)value;
    return value == 0 ? 0 : 1;
}

int mbtowc(wchar_t *wide_character, const char *bytes, size_t length) {
    unsigned char value;
    if (bytes == 0) return 0; // reset the stateless C-locale conversion state
    if (length == 0) return -1;
    value = (unsigned char)bytes[0];
    if (value > 0x7f) { errno = EILSEQ; return -1; }
    if (wide_character != 0) *wide_character = (wchar_t)value;
    return value == 0 ? 0 : 1;
}

int wctomb(char *buffer, wchar_t wide_character) {
    if (buffer == 0) return 0;
    if ((unsigned int)wide_character > 0x7f) { errno = EILSEQ; return -1; }
    buffer[0] = (char)wide_character;
    return 1;
}

size_t wcrtomb(char *buffer, wchar_t wide_character, mbstate_t *state) {
    (void)state;
    if (buffer == 0) return 1;
    if ((unsigned int)wide_character > 0x7f) { errno = EILSEQ; return (size_t)-1; }
    buffer[0] = (char)wide_character;
    return 1;
}

wchar_t *wmemcpy(wchar_t *destination, const wchar_t *source, size_t count) {
    size_t index;
    for (index = 0; index < count; ++index) destination[index] = source[index];
    return destination;
}

wint_t towlower(wint_t wide_character) {
    if (wide_character <= 0x7f) return (wint_t)tolower((int)wide_character);
    return wide_character;
}

size_t wcslen(const wchar_t *string) {
    size_t length = 0;
    while (string[length] != 0) ++length;
    return length;
}

wchar_t *wcscat(wchar_t *destination, const wchar_t *source) {
    wchar_t *result = destination;
    while (*destination != 0) ++destination;
    while ((*destination++ = *source++) != 0) {}
    return result;
}

wctype_t wctype(const char *name) {
    static const char *const classes[] = {
        "alnum", "alpha", "blank", "cntrl", "digit", "graph",
        "lower", "print", "punct", "space", "upper", "xdigit"
    };
    unsigned int index;
    if (name == 0) return 0;
    for (index = 0; index < sizeof(classes) / sizeof(classes[0]); ++index)
        if (strcmp(name, classes[index]) == 0) return index + 1;
    return 0;
}

int iswctype(wint_t character, wctype_t character_class) {
    int c;
    if (character > 0x7f || character_class == 0 || character_class > 12) return 0;
    c = (int)character;
    switch (character_class) {
    case 1: return isalnum(c) != 0;
    case 2: return isalpha(c) != 0;
    case 3: return c == ' ' || c == '\t';
    case 4: return iscntrl(c) != 0;
    case 5: return isdigit(c) != 0;
    case 6: return isgraph(c) != 0;
    case 7: return islower(c) != 0;
    case 8: return isprint(c) != 0;
    case 9: return ispunct(c) != 0;
    case 10: return isspace(c) != 0;
    case 11: return isupper(c) != 0;
    case 12: return isxdigit(c) != 0;
    default: return 0;
    }
}
