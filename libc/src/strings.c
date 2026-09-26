#include <ctype.h>
#include <stddef.h>
#include <strings.h>

int strcasecmp(const char *left, const char *right) {
    unsigned char a, b;
    do {
        a = (unsigned char)*left++;
        b = (unsigned char)*right++;
        if (tolower(a) != tolower(b)) return tolower(a) - tolower(b);
    } while (a != 0);
    return 0;
}

int strncasecmp(const char *left, const char *right, size_t length) {
    unsigned char a, b;
    while (length-- != 0) {
        a = (unsigned char)*left++;
        b = (unsigned char)*right++;
        if (tolower(a) != tolower(b)) return tolower(a) - tolower(b);
        if (a == 0) return 0;
    }
    return 0;
}
