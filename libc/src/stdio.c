#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include "stdio_internal.h"

static FILE standard_input = {STDIN_FILENO, -1, 0, 0, 0};
static FILE standard_output = {STDOUT_FILENO, -1, 0, 0, 0};
static FILE standard_error = {STDERR_FILENO, -1, 0, 0, 0};
FILE *stdin = &standard_input;
FILE *stdout = &standard_output;
FILE *stderr = &standard_error;

int fgetc(FILE *stream) {
    unsigned char byte;
    ssize_t result;
    if (stream == 0) { errno = EINVAL; return EOF; }
    if (stream->pushback >= 0) {
        int character = stream->pushback;
        stream->pushback = -1;
        stream->eof = 0;
        return character;
    }
    result = read(stream->fd, &byte, 1);
    if (result == 0) { stream->eof = 1; return EOF; }
    if (result < 0) { stream->error = 1; return EOF; }
    return byte;
}

int fputc(int character, FILE *stream) {
    unsigned char byte = (unsigned char)character;
    ssize_t result;
    if (stream == 0) { errno = EINVAL; return EOF; }
    result = write(stream->fd, &byte, 1);
    if (result != 1) { stream->error = 1; return EOF; }
    return byte;
}

int fputs(const char *text, FILE *stream) {
    size_t length;
    ssize_t result;
    if (text == 0 || stream == 0) { errno = EINVAL; return EOF; }
    length = strlen(text);
    result = write(stream->fd, text, length);
    if (result < 0 || (size_t)result != length) {
        stream->error = 1;
        return EOF;
    }
    return 0;
}

int puts(const char *text) {
    if (fputs(text, stdout) == EOF || fputc('\n', stdout) == EOF) return EOF;
    return 0;
}

char *fgets(char *buffer, int size, FILE *stream) {
    int count = 0;
    if (buffer == 0 || stream == 0 || size <= 0) { errno = EINVAL; return 0; }
    while (count + 1 < size) {
        int character = fgetc(stream);
        if (character == EOF) break;
        buffer[count++] = (char)character;
        if (character == '\n') break;
    }
    if (count == 0 && stream->eof) return 0;
    buffer[count] = '\0';
    return buffer;
}

size_t fread(void *buffer, size_t size, size_t count, FILE *stream) {
    size_t total;
    size_t done = 0;
    if (size == 0 || count == 0) return 0;
    if (stream == 0 || buffer == 0 || count > SIZE_MAX / size) {
        errno = EINVAL;
        return 0;
    }
    total = size * count;
    if (stream->pushback >= 0 && total != 0) {
        ((unsigned char *)buffer)[done++] = (unsigned char)stream->pushback;
        stream->pushback = -1;
    }
    while (done < total) {
        size_t request = total - done;
        ssize_t amount = read(stream->fd, (unsigned char *)buffer + done, request);
        if (amount == 0) { stream->eof = 1; break; }
        if (amount < 0) { stream->error = 1; break; }
        done += (size_t)amount;
        if ((size_t)amount < request) break;
    }
    return done / size;
}

size_t fwrite(const void *buffer, size_t size, size_t count, FILE *stream) {
    size_t total;
    ssize_t amount;
    if (size == 0 || count == 0) return 0;
    if (stream == 0 || buffer == 0 || count > SIZE_MAX / size) {
        errno = EINVAL;
        return 0;
    }
    total = size * count;
    amount = write(stream->fd, buffer, total);
    if (amount < 0) { stream->error = 1; return 0; }
    if ((size_t)amount < total) stream->error = 1;
    return (size_t)amount / size;
}

static int mode_flags(const char *mode) {
    if (mode == 0 || mode[0] == 0) return -1;
    if (mode[0] == 'r') {
        if (mode[1] == 0 || (mode[1] == 'b' && mode[2] == 0)) return O_RDONLY;
        if ((mode[1] == '+' && mode[2] == 0) ||
            (mode[1] == 'b' && mode[2] == '+' && mode[3] == 0) ||
            (mode[1] == '+' && mode[2] == 'b' && mode[3] == 0)) return O_RDWR;
    }
    return -1;
}

FILE *fdopen(int fd, const char *mode) {
    FILE *stream;
    if (fd < 0 || mode_flags(mode) < 0) { errno = EINVAL; return 0; }
    stream = (FILE *)malloc(sizeof(*stream));
    if (stream == 0) return 0;
    stream->fd = fd;
    stream->pushback = -1;
    stream->eof = 0;
    stream->error = 0;
    stream->owned = 1;
    return stream;
}

FILE *fopen(const char *path, const char *mode) {
    int flags = mode_flags(mode);
    int fd;
    FILE *stream;
    if (path == 0) { errno = EFAULT; return 0; }
    if (flags < 0) { errno = ENOSYS; return 0; }
    fd = open(path, flags);
    if (fd < 0) return 0;
    stream = fdopen(fd, mode);
    if (stream == 0) (void)close(fd);
    return stream;
}

int fclose(FILE *stream) {
    int result = 0;
    if (stream == 0) { errno = EINVAL; return EOF; }
    if (stream->owned) {
        result = close(stream->fd);
        free(stream);
    }
    return result < 0 ? EOF : 0;
}

int fflush(FILE *stream) {
    (void)stream;
    return 0;
}

int fileno(FILE *stream) {
    if (stream == 0) { errno = EINVAL; return -1; }
    return stream->fd;
}

int feof(FILE *stream) { return stream != 0 && stream->eof != 0; }
int ferror(FILE *stream) { return stream != 0 && stream->error != 0; }
void clearerr(FILE *stream) { if (stream != 0) { stream->eof = 0; stream->error = 0; } }

int ungetc(int character, FILE *stream) {
    if (character == EOF || stream == 0 || stream->pushback >= 0) return EOF;
    stream->pushback = (unsigned char)character;
    stream->eof = 0;
    return (unsigned char)character;
}

int fseek(FILE *stream, long offset, int whence) {
    (void)stream; (void)offset; (void)whence;
    errno = ENOSYS;
    return -1;
}
long ftell(FILE *stream) {
    (void)stream;
    errno = ENOSYS;
    return -1;
}

void perror(const char *prefix) {
    if (prefix != 0 && *prefix != 0) {
        (void)fputs(prefix, stderr);
        (void)fputs(": ", stderr);
    }
    (void)fputs(strerror(errno), stderr);
    (void)fputc('\n', stderr);
}

int vfprintf(FILE *stream, const char *format, va_list arguments) {
    va_list measure;
    va_list render;
    int required;
    char *buffer;
    size_t written;
    if (stream == 0) { errno = EINVAL; return -1; }
    va_copy(measure, arguments);
    required = vsnprintf(0, 0, format, measure);
    va_end(measure);
    if (required < 0) return -1;
    buffer = (char *)malloc((size_t)required + 1);
    if (buffer == 0) return -1;
    va_copy(render, arguments);
    if (vsnprintf(buffer, (size_t)required + 1, format, render) < 0) {
        va_end(render);
        free(buffer);
        return -1;
    }
    va_end(render);
    written = fwrite(buffer, 1, (size_t)required, stream);
    free(buffer);
    if (written != (size_t)required) return -1;
    return required;
}

int fprintf(FILE *stream, const char *format, ...) {
    va_list arguments;
    int result;
    va_start(arguments, format);
    result = vfprintf(stream, format, arguments);
    va_end(arguments);
    return result;
}

int vprintf(const char *format, va_list arguments) { return vfprintf(stdout, format, arguments); }
int printf(const char *format, ...) {
    va_list arguments;
    int result;
    va_start(arguments, format);
    result = vfprintf(stdout, format, arguments);
    va_end(arguments);
    return result;
}
