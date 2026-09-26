#ifndef _SBOS_STRING_H
#define _SBOS_STRING_H
#include <stddef.h>

void *memcpy(void *destination, const void *source, size_t length);
void *memchr(const void *bytes, int value, size_t length);
void *memmove(void *destination, const void *source, size_t length);
void bcopy(const void *source, void *destination, size_t length);
void *memset(void *destination, int value, size_t length);
int memcmp(const void *left, const void *right, size_t length);
size_t strlen(const char *text);
size_t strnlen(const char *text, size_t limit);
int strcmp(const char *left, const char *right);
int strncmp(const char *left, const char *right, size_t limit);
char *strcpy(char *destination, const char *source);
char *strncpy(char *destination, const char *source, size_t limit);
char *strcat(char *destination, const char *source);
char *strchr(const char *text, int character);
char *strrchr(const char *text, int character);
char *strstr(const char *text, const char *needle);
char *strdup(const char *text);
char *strerror(int error);

#endif
