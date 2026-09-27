#ifndef _SBOS_STDIO_H
#define _SBOS_STDIO_H
#include <stdarg.h>
#include <stddef.h>
#include <sys/types.h>

typedef struct __sbos_FILE FILE;
/* SBOS streams are currently unbuffered; kept for Gnulib-compatible callers. */
size_t __fpending(FILE *stream);
void __fpurge(FILE *stream);
size_t __freadahead(FILE *stream);
int __freading(FILE *stream);
void __fseterr(FILE *stream);
#define EOF (-1)
#define BUFSIZ 4096
#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2
#define _IONBF 0
#define _IOLBF 1
#define _IOFBF 2

extern FILE *stdin;
extern FILE *stdout;
extern FILE *stderr;

int printf(const char *format, ...);
int fprintf(FILE *stream, const char *format, ...);
int sprintf(char *buffer, const char *format, ...);
int snprintf(char *buffer, size_t size, const char *format, ...);
int vprintf(const char *format, va_list arguments);
int vfprintf(FILE *stream, const char *format, va_list arguments);
int vsprintf(char *buffer, const char *format, va_list arguments);
int vsnprintf(char *buffer, size_t size, const char *format, va_list arguments);
int puts(const char *text);
int fputs(const char *text, FILE *stream);
int putchar(int character);
int getchar(void);
int fputc(int character, FILE *stream);
int fgetc(FILE *stream);
char *fgets(char *buffer, int size, FILE *stream);
size_t fread(void *buffer, size_t size, size_t count, FILE *stream);
size_t fwrite(const void *buffer, size_t size, size_t count, FILE *stream);
FILE *fopen(const char *path, const char *mode);
FILE *fdopen(int fd, const char *mode);
FILE *freopen(const char *path, const char *mode, FILE *stream);
int fclose(FILE *stream);
int fflush(FILE *stream);
int setvbuf(FILE *stream, char *buffer, int mode, size_t size);
int fileno(FILE *stream);
int feof(FILE *stream);
int ferror(FILE *stream);
void clearerr(FILE *stream);
int ungetc(int character, FILE *stream);
int fseek(FILE *stream, long offset, int whence);
long ftell(FILE *stream);
void perror(const char *prefix);
int rename(const char *old_path, const char *new_path);

#define getc(stream) fgetc(stream)
#define putc(character, stream) fputc((character), (stream))
#define getchar() fgetc(stdin)
#define putchar(character) fputc((character), stdout)

#endif
