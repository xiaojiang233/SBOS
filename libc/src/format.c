#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stddef.h>

typedef struct {
    char *buffer;
    size_t capacity;
    size_t length;
} output_buffer;

enum length_modifier { LEN_DEFAULT, LEN_HH, LEN_H, LEN_L, LEN_LL, LEN_Z };

static void emit_char(output_buffer *out, char character) {
    if (out->capacity != 0 && out->length + 1 < out->capacity)
        out->buffer[out->length] = character;
    if (out->length != SIZE_MAX) ++out->length;
}

static void emit_repeat(output_buffer *out, char character, int count) {
    while (count-- > 0) emit_char(out, character);
}

static int parse_number(const char **cursor) {
    int value = 0;
    while (**cursor >= '0' && **cursor <= '9') {
        int digit = *(*cursor)++ - '0';
        if (value > (INT_MAX - digit) / 10) return INT_MAX;
        value = value * 10 + digit;
    }
    return value;
}

static void emit_field(output_buffer *out, const char *text, size_t length,
                       int width, int left, char padding) {
    int pad = width > (int)length ? width - (int)length : 0;
    if (!left) emit_repeat(out, padding, pad);
    while (length-- != 0) emit_char(out, *text++);
    if (left) emit_repeat(out, ' ', pad);
}

static void emit_integer(output_buffer *out, uint64_t value, int negative,
                        unsigned base, int uppercase, int width,
                        int precision, int left, int zero, int plus,
                        int space, int alternate, int pointer) {
    char digits[65];
    const char *alphabet = uppercase ? "0123456789ABCDEF" : "0123456789abcdef";
    char sign = negative ? '-' : (plus ? '+' : (space ? ' ' : 0));
    char prefix[2];
    int prefix_length = 0;
    size_t digit_count = 0;
    size_t zeros;
    int padding;
    size_t index;

    if (value == 0 && precision == 0 && !pointer) {
        digit_count = 0;
    } else {
        do {
            digits[digit_count++] = alphabet[value % base];
            value /= base;
        } while (value != 0);
    }
    if (pointer || (alternate && base == 16 && digit_count != 0)) {
        prefix[0] = '0';
        prefix[1] = uppercase ? 'X' : 'x';
        prefix_length = 2;
    } else if (alternate && base == 8 &&
               (digit_count == 0 || digits[digit_count - 1] != '0')) {
        prefix[0] = '0';
        prefix_length = 1;
    }
    zeros = precision > (int)digit_count ? (size_t)precision - digit_count : 0;
    padding = width - (int)digit_count - (int)zeros - prefix_length - (sign != 0);
    if (!left && !(zero && precision < 0)) emit_repeat(out, ' ', padding);
    if (sign != 0) emit_char(out, sign);
    for (index = 0; index < (size_t)prefix_length; ++index) emit_char(out, prefix[index]);
    if (!left && zero && precision < 0) emit_repeat(out, '0', padding);
    emit_repeat(out, '0', (int)zeros);
    while (digit_count != 0) emit_char(out, digits[--digit_count]);
    if (left) emit_repeat(out, ' ', padding);
}

static int64_t signed_argument(va_list *arguments, enum length_modifier length) {
    switch (length) {
    case LEN_L: return va_arg(*arguments, long);
    case LEN_LL: return va_arg(*arguments, long long);
    case LEN_Z: return va_arg(*arguments, ptrdiff_t);
    case LEN_H: return (short)va_arg(*arguments, int);
    case LEN_HH: return (signed char)va_arg(*arguments, int);
    default: return va_arg(*arguments, int);
    }
}

static uint64_t unsigned_argument(va_list *arguments, enum length_modifier length) {
    switch (length) {
    case LEN_L: return va_arg(*arguments, unsigned long);
    case LEN_LL: return va_arg(*arguments, unsigned long long);
    case LEN_Z: return va_arg(*arguments, size_t);
    case LEN_H: return (unsigned short)va_arg(*arguments, unsigned int);
    case LEN_HH: return (unsigned char)va_arg(*arguments, unsigned int);
    default: return va_arg(*arguments, unsigned int);
    }
}

int vsnprintf(char *buffer, size_t size, const char *format, va_list arguments) {
    output_buffer out = {buffer, size, 0};
    va_list args;
    const char *cursor = format;
    if (format == 0 || (buffer == 0 && size != 0)) { errno = EINVAL; return -1; }
    va_copy(args, arguments);
    while (*cursor != 0) {
        int left = 0, zero = 0, plus = 0, space = 0, alternate = 0;
        int width = 0, precision = -1;
        enum length_modifier length = LEN_DEFAULT;
        char conversion;
        if (*cursor != '%') { emit_char(&out, *cursor++); continue; }
        ++cursor;
        if (*cursor == '%') { emit_char(&out, *cursor++); continue; }
        for (;;) {
            if (*cursor == '-') left = 1;
            else if (*cursor == '0') zero = 1;
            else if (*cursor == '+') plus = 1;
            else if (*cursor == ' ') space = 1;
            else if (*cursor == '#') alternate = 1;
            else break;
            ++cursor;
        }
        if (*cursor == '*') {
            width = va_arg(args, int);
            ++cursor;
            if (width < 0) { left = 1; width = width == INT_MIN ? INT_MAX : -width; }
        } else {
            width = parse_number(&cursor);
        }
        if (*cursor == '.') {
            ++cursor;
            precision = 0;
            if (*cursor == '*') {
                precision = va_arg(args, int);
                ++cursor;
                if (precision < 0) precision = -1;
            } else {
                precision = parse_number(&cursor);
            }
        }
        if (*cursor == 'h') { ++cursor; length = *cursor == 'h' ? (++cursor, LEN_HH) : LEN_H; }
        else if (*cursor == 'l') { ++cursor; length = *cursor == 'l' ? (++cursor, LEN_LL) : LEN_L; }
        else if (*cursor == 'z') { ++cursor; length = LEN_Z; }
        conversion = *cursor;
        if (conversion == 0) break;
        ++cursor;
        switch (conversion) {
        case 'd': case 'i': {
            int64_t number = signed_argument(&args, length);
            int negative = number < 0;
            uint64_t magnitude = negative ? 0ULL - (uint64_t)number : (uint64_t)number;
            emit_integer(&out, magnitude, negative, 10, 0, width, precision,
                         left, zero, plus, space, 0, 0);
            break;
        }
        case 'u': case 'o': case 'x': case 'X': {
            unsigned base = conversion == 'o' ? 8U :
                            (conversion == 'u' ? 10U : 16U);
            emit_integer(&out, unsigned_argument(&args, length), 0, base,
                         conversion == 'X', width, precision, left, zero, 0, 0,
                         alternate, 0);
            break;
        }
        case 'p':
            emit_integer(&out, (uint64_t)(uintptr_t)va_arg(args, void *), 0,
                         16, 0, width, precision, left, zero, 0, 0, 0, 1);
            break;
        case 'c': {
            char character = (char)va_arg(args, int);
            emit_field(&out, &character, 1, width, left, ' ');
            break;
        }
        case 's': {
            const char *text = va_arg(args, const char *);
            size_t length_text = 0;
            if (text == 0) text = "(null)";
            while (text[length_text] != 0 &&
                   (precision < 0 || length_text < (size_t)precision)) ++length_text;
            emit_field(&out, text, length_text, width, left, ' ');
            break;
        }
        case 'n': {
            void *destination = va_arg(args, void *);
            if (destination == 0) { va_end(args); errno = EINVAL; return -1; }
            if (length == LEN_HH) *(signed char *)destination = (signed char)out.length;
            else if (length == LEN_H) *(short *)destination = (short)out.length;
            else if (length == LEN_L) *(long *)destination = (long)out.length;
            else if (length == LEN_LL) *(long long *)destination = (long long)out.length;
            else if (length == LEN_Z) *(size_t *)destination = out.length;
            else *(int *)destination = (int)out.length;
            break;
        }
        default:
            va_end(args);
            errno = EINVAL;
            return -1;
        }
    }
    va_end(args);
    if (size != 0) buffer[out.length < size ? out.length : size - 1] = '\0';
    if (out.length > INT_MAX) { errno = EOVERFLOW; return -1; }
    return (int)out.length;
}

int snprintf(char *buffer, size_t size, const char *format, ...) {
    va_list arguments;
    int result;
    va_start(arguments, format);
    result = vsnprintf(buffer, size, format, arguments);
    va_end(arguments);
    return result;
}

int vsprintf(char *buffer, const char *format, va_list arguments) {
    return vsnprintf(buffer, SIZE_MAX, format, arguments);
}

int sprintf(char *buffer, const char *format, ...) {
    va_list arguments;
    int result;
    va_start(arguments, format);
    result = vsprintf(buffer, format, arguments);
    va_end(arguments);
    return result;
}
