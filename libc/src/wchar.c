#include <ctype.h>
#include <stddef.h>
#include <wchar.h>

/* The current SBOS locale is the single-byte C locale. */
size_t mbrtowc(wchar_t *wide_character, const char *bytes, size_t length,
               mbstate_t *state) {
    unsigned char value;
    (void)state;
    if (bytes == 0) {
        if (wide_character != 0) *wide_character = 0;
        return 1;
    }
    if (length == 0) return (size_t)-2;
    value = (unsigned char)bytes[0];
    if (wide_character != 0) *wide_character = (wchar_t)value;
    return value == 0 ? 0 : 1;
}

wint_t towlower(wint_t wide_character) {
    if (wide_character <= 0x7f) return (wint_t)tolower((int)wide_character);
    return wide_character;
}
