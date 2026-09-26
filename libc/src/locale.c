#include <errno.h>
#include <locale.h>
#include <string.h>

static char decimal_point[] = ".";
static char empty[] = "";
static struct lconv c_locale = {
    decimal_point, empty, empty, empty, empty, decimal_point, empty, empty,
    empty, empty, 2, 2, 1, 0, 1, 0, 1, 1
};

char *setlocale(int category, const char *locale) {
    (void)category;
    if (locale == 0 || locale[0] == '\0' || strcmp(locale, "C") == 0 || strcmp(locale, "POSIX") == 0)
        return "C";
    errno = ENOSYS;
    return 0;
}

struct lconv *localeconv(void) { return &c_locale; }
