#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

void *memchr(const void *bytes, int value, size_t length) {
    const unsigned char *cursor = (const unsigned char *)bytes;
    unsigned char needle = (unsigned char)value;
    size_t index;
    for (index = 0; index < length; ++index) {
        if (cursor[index] == needle) return (void *)(cursor + index);
    }
    return 0;
}

void bcopy(const void *source, void *destination, size_t length) {
    (void)memmove(destination, source, length);
}


void *memcpy(void *destination, const void *source, size_t length) {
    unsigned char *out = (unsigned char *)destination;
    const unsigned char *in = (const unsigned char *)source;
    size_t index;
    for (index = 0; index < length; ++index) out[index] = in[index];
    return destination;
}

void *memmove(void *destination, const void *source, size_t length) {
    uintptr_t out = (uintptr_t)destination;
    uintptr_t in = (uintptr_t)source;
    size_t index;
    if (out <= in || out - in >= length) {
        for (index = 0; index < length; ++index)
            ((unsigned char *)destination)[index] = ((const unsigned char *)source)[index];
    } else {
        for (index = length; index != 0; --index)
            ((unsigned char *)destination)[index - 1] = ((const unsigned char *)source)[index - 1];
    }
    return destination;
}

void *memset(void *destination, int value, size_t length) {
    unsigned char *out = (unsigned char *)destination;
    size_t index;
    for (index = 0; index < length; ++index) out[index] = (unsigned char)value;
    return destination;
}

int memcmp(const void *left, const void *right, size_t length) {
    const unsigned char *a = (const unsigned char *)left;
    const unsigned char *b = (const unsigned char *)right;
    size_t index;
    for (index = 0; index < length; ++index)
        if (a[index] != b[index]) return (int)a[index] - (int)b[index];
    return 0;
}

size_t strlen(const char *text) {
    size_t length = 0;
    while (text[length] != '\0') ++length;
    return length;
}

size_t strnlen(const char *text, size_t limit) {
    size_t length = 0;
    while (length < limit && text[length] != '\0') ++length;
    return length;
}

int strcmp(const char *left, const char *right) {
    while (*left && (unsigned char)*left == (unsigned char)*right) {
        ++left;
        ++right;
    }
    return (int)(unsigned char)*left - (int)(unsigned char)*right;
}

int strncmp(const char *left, const char *right, size_t limit) {
    size_t index;
    for (index = 0; index < limit; ++index) {
        unsigned char a = (unsigned char)left[index];
        unsigned char b = (unsigned char)right[index];
        if (a != b || a == 0) return (int)a - (int)b;
    }
    return 0;
}

char *strcpy(char *destination, const char *source) {
    char *result = destination;
    while ((*destination++ = *source++) != '\0') {}
    return result;
}

char *strncpy(char *destination, const char *source, size_t limit) {
    size_t index = 0;
    while (index < limit && source[index] != '\0') {
        destination[index] = source[index];
        ++index;
    }
    while (index < limit) destination[index++] = '\0';
    return destination;
}

char *strcat(char *destination, const char *source) {
    strcpy(destination + strlen(destination), source);
    return destination;
}

char *strchr(const char *text, int character) {
    unsigned char needle = (unsigned char)character;
    for (;;) {
        if ((unsigned char)*text == needle) return (char *)text;
        if (*text == '\0') return 0;
        ++text;
    }
}

char *strrchr(const char *text, int character) {
    const char *found = 0;
    unsigned char needle = (unsigned char)character;
    do {
        if ((unsigned char)*text == needle) found = text;
    } while (*text++ != '\0');
    return (char *)found;
}

char *strstr(const char *text, const char *needle) {
    size_t length = strlen(needle);
    if (length == 0) return (char *)text;
    while (*text) {
        if (*text == *needle && strncmp(text, needle, length) == 0)
            return (char *)text;
        ++text;
    }
    return 0;
}

char *strdup(const char *text) {
    size_t length = strlen(text) + 1;
    char *copy = (char *)malloc(length);
    if (copy != 0) memcpy(copy, text, length);
    return copy;
}
