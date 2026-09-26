#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#define PAGE_SIZE 4096U
#define MAX_MAPPING (16U * 1024U * 1024U)
#define ATEXIT_LIMIT 32

typedef struct {
    size_t mapping_size;
    size_t requested_size;
} allocation_header;

static void (*exit_functions[ATEXIT_LIMIT])(void);
static size_t exit_function_count;
static unsigned int random_state = 1;
#if !defined(SBOS_NO_ENVIRONMENT)
static char *initial_environment[1] = {0};
static size_t environment_count;
static size_t environment_capacity = 1;
static int environment_is_dynamic;
char **environ = initial_environment;

void __sbos_init_environment(char **initial) {
    size_t count = 0;
    environ = initial != 0 ? initial : initial_environment;
    while (count < 256 && environ[count] != 0) ++count;
    environment_count = count;
    environment_capacity = count + 1;
    environment_is_dynamic = 0;
}
#endif

void *malloc(size_t size) {
    size_t needed;
    size_t mapping_size;
    allocation_header *header;
    void *mapping;
    if (size == 0) size = 1;
    if (size > SIZE_MAX - sizeof(*header)) { errno = ENOMEM; return 0; }
    needed = size + sizeof(*header);
    if (needed > SIZE_MAX - (PAGE_SIZE - 1)) { errno = ENOMEM; return 0; }
    mapping_size = (needed + PAGE_SIZE - 1) & ~(PAGE_SIZE - 1);
    if (mapping_size > MAX_MAPPING) { errno = ENOMEM; return 0; }
    mapping = mmap(0, mapping_size, PROT_READ | PROT_WRITE,
                   MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (mapping == MAP_FAILED) return 0;
    header = (allocation_header *)mapping;
    header->mapping_size = mapping_size;
    header->requested_size = size;
    return (void *)(header + 1);
}

void free(void *pointer) {
    allocation_header *header;
    if (pointer == 0) return;
    header = ((allocation_header *)pointer) - 1;
    (void)munmap((void *)header, header->mapping_size);
}

void *calloc(size_t count, size_t size) {
    size_t total;
    unsigned char *memory;
    size_t index;
    if (size != 0 && count > SIZE_MAX / size) { errno = ENOMEM; return 0; }
    total = count * size;
    memory = (unsigned char *)malloc(total);
    if (memory == 0) return 0;
    for (index = 0; index < total; ++index) memory[index] = 0;
    return memory;
}

void *realloc(void *pointer, size_t size) {
    allocation_header *header;
    void *replacement;
    size_t copied;
    if (pointer == 0) return malloc(size);
    if (size == 0) { free(pointer); return 0; }
    header = ((allocation_header *)pointer) - 1;
    replacement = malloc(size);
    if (replacement == 0) return 0;
    copied = header->requested_size < size ? header->requested_size : size;
    memcpy(replacement, pointer, copied);
    free(pointer);
    return replacement;
}

int atexit(void (*function)(void)) {
    if (function == 0 || exit_function_count == ATEXIT_LIMIT) {
        errno = ENOMEM;
        return -1;
    }
    exit_functions[exit_function_count++] = function;
    return 0;
}

void exit(int status) {
    while (exit_function_count != 0) exit_functions[--exit_function_count]();
    _exit(status);
}

void abort(void) { _exit(128 + SIGABRT); }

static int digit_value(int character) {
    if (character >= '0' && character <= '9') return character - '0';
    if (character >= 'a' && character <= 'z') return character - 'a' + 10;
    if (character >= 'A' && character <= 'Z') return character - 'A' + 10;
    return -1;
}

static int parse_integer(const char *text, char **end, int base,
                         int *negative, unsigned long *magnitude,
                         int *overflow) {
    const char *cursor = text;
    const char *digits;
    unsigned long value = 0;
    int digit;
    while (isspace((unsigned char)*cursor)) ++cursor;
    *negative = 0;
    if (*cursor == '+' || *cursor == '-') {
        *negative = *cursor == '-';
        ++cursor;
    }
    if (base != 0 && (base < 2 || base > 36)) {
        return -1;
    }
    if ((base == 0 || base == 16) && cursor[0] == '0' &&
        (cursor[1] == 'x' || cursor[1] == 'X') &&
        digit_value((unsigned char)cursor[2]) >= 0 &&
        digit_value((unsigned char)cursor[2]) < 16) {
        cursor += 2;
        base = 16;
    } else if (base == 0) {
        base = cursor[0] == '0' ? 8 : 10;
    }
    digits = cursor;
    *overflow = 0;
    while ((digit = digit_value((unsigned char)*cursor)) >= 0 && digit < base) {
        if (value > (ULONG_MAX - (unsigned long)digit) / (unsigned long)base)
            *overflow = 1;
        else if (!*overflow)
            value = value * (unsigned long)base + (unsigned long)digit;
        ++cursor;
    }
    if (cursor == digits) { if (end) *end = (char *)text; return 0; }
    if (end) *end = (char *)cursor;
    *magnitude = value;
    return 1;
}

unsigned long strtoul(const char *text, char **end, int base) {
    unsigned long magnitude = 0;
    int negative = 0;
    int overflow = 0;
    int parsed;
    if (text == 0) { errno = EINVAL; if (end) *end = 0; return 0; }
    parsed = parse_integer(text, end, base, &negative, &magnitude, &overflow);
    if (parsed < 0) { errno = EINVAL; if (end) *end = (char *)text; return 0; }
    if (parsed == 0) return 0;
    if (overflow) { errno = ERANGE; return ULONG_MAX; }
    return negative ? 0UL - magnitude : magnitude;
}

long strtol(const char *text, char **end, int base) {
    unsigned long magnitude = 0;
    unsigned long limit;
    int negative = 0;
    int overflow = 0;
    int parsed;
    if (text == 0) { errno = EINVAL; if (end) *end = 0; return 0; }
    parsed = parse_integer(text, end, base, &negative, &magnitude, &overflow);
    if (parsed < 0) { errno = EINVAL; if (end) *end = (char *)text; return 0; }
    if (parsed == 0) return 0;
    limit = negative ? (unsigned long)LONG_MAX + 1UL : (unsigned long)LONG_MAX;
    if (overflow || magnitude > limit) {
        errno = ERANGE;
        return negative ? LONG_MIN : LONG_MAX;
    }
    if (negative) return magnitude == (unsigned long)LONG_MAX + 1UL ? LONG_MIN : -(long)magnitude;
    return (long)magnitude;
}

int atoi(const char *text) { return (int)strtol(text, 0, 10); }
int abs(int value) { return value < 0 ? -value : value; }
long labs(long value) { return value < 0 ? -value : value; }

#if !defined(SBOS_NO_ENVIRONMENT)
static size_t environment_index(const char *name, size_t length) {
    size_t index;
    for (index = 0; index < environment_count; ++index) {
        if (strncmp(environ[index], name, length) == 0 && environ[index][length] == '=')
            return index;
    }
    return environment_count;
}

char *getenv(const char *name) {
    size_t length;
    size_t index;
    if (name == 0 || *name == 0 || strchr(name, '=') != 0) return 0;
    length = strlen(name);
    index = environment_index(name, length);
    return index == environment_count ? 0 : strchr(environ[index], '=') + 1;
}

static int environment_reserve(size_t wanted) {
    char **replacement;
    size_t capacity;
    size_t index;
    if (wanted <= environment_capacity) return 0;
    capacity = environment_capacity < 8 ? 8 : environment_capacity;
    while (capacity < wanted) {
        if (capacity > SIZE_MAX / 2) { errno = ENOMEM; return -1; }
        capacity *= 2;
    }
    replacement = (char **)malloc(capacity * sizeof(char *));
    if (replacement == 0) return -1;
    for (index = 0; index < environment_count; ++index) replacement[index] = environ[index];
    replacement[environment_count] = 0;
    if (environment_is_dynamic) free(environ);
    environ = replacement;
    environment_capacity = capacity;
    environment_is_dynamic = 1;
    return 0;
}

static int install_environment(char *assignment, int overwrite) {
    char *separator = strchr(assignment, '=');
    size_t name_length;
    size_t index;
    if (separator == 0 || separator == assignment) { errno = EINVAL; return -1; }
    name_length = (size_t)(separator - assignment);
    index = environment_index(assignment, name_length);
    if (index != environment_count && !overwrite) return 0;
    if (index == environment_count) {
        if (environment_reserve(environment_count + 2) < 0) return -1;
        environ[environment_count++] = assignment;
        environ[environment_count] = 0;
    } else {
        environ[index] = assignment;
    }
    return 0;
}

int setenv(const char *name, const char *value, int overwrite) {
    size_t name_length;
    size_t value_length;
    char *assignment;
    if (name == 0 || value == 0 || *name == 0 || strchr(name, '=') != 0) {
        errno = EINVAL;
        return -1;
    }
    name_length = strlen(name);
    if (environment_index(name, name_length) != environment_count && !overwrite) return 0;
    value_length = strlen(value);
    if (name_length > SIZE_MAX - value_length - 2) { errno = ENOMEM; return -1; }
    assignment = (char *)malloc(name_length + value_length + 2);
    if (assignment == 0) return -1;
    memcpy(assignment, name, name_length);
    assignment[name_length] = '=';
    memcpy(assignment + name_length + 1, value, value_length + 1);
    if (install_environment(assignment, 1) < 0) { free(assignment); return -1; }
    return 0;
}

int unsetenv(const char *name) {
    size_t length;
    size_t index;
    if (name == 0 || *name == 0 || strchr(name, '=') != 0) { errno = EINVAL; return -1; }
    length = strlen(name);
    index = environment_index(name, length);
    if (index == environment_count) return 0;
    while (index + 1 < environment_count) {
        environ[index] = environ[index + 1];
        ++index;
    }
    environ[--environment_count] = 0;
    return 0;
}

int putenv(char *assignment) {
    return assignment == 0 ? (errno = EINVAL, -1) : install_environment(assignment, 1);
}
#endif

int system(const char *command) {
    (void)command;
    errno = ENOSYS;
    return -1;
}

int rand(void) {
    random_state = random_state * 1103515245U + 12345U;
    return (int)((random_state >> 16) & RAND_MAX);
}
void srand(unsigned int seed) { random_state = seed; }

char *strerror(int error) {
    switch (error) {
    case 0: return "Success";
    case EPERM: return "Operation not permitted";
    case ENOENT: return "No such file or directory";
    case EINTR: return "Interrupted system call";
    case EIO: return "Input/output error";
    case EBADF: return "Bad file descriptor";
    case ECHILD: return "No child processes";
    case EAGAIN: return "Resource temporarily unavailable";
    case ENOMEM: return "Cannot allocate memory";
    case EACCES: return "Permission denied";
    case EFAULT: return "Bad address";
    case EEXIST: return "File exists";
    case ENOTDIR: return "Not a directory";
    case EISDIR: return "Is a directory";
    case EINVAL: return "Invalid argument";
    case EMFILE: return "Too many open files";
    case ENOTTY: return "Inappropriate ioctl for device";
    case ENOSPC: return "No space left on device";
    case ERANGE: return "Result out of range";
    case ENOSYS: return "Function not implemented";
    case ETIMEDOUT: return "Connection timed out";
    default: return "Unknown error";
    }
}

static unsigned char *element(void *base, size_t size, size_t index) {
    return (unsigned char *)base + size * index;
}

static void swap_elements(unsigned char *left, unsigned char *right, size_t size) {
    size_t index;
    for (index = 0; index < size; ++index) {
        unsigned char temporary = left[index];
        left[index] = right[index];
        right[index] = temporary;
    }
}

static void sift_down(void *base, size_t root, size_t end, size_t size,
                      int (*compare)(const void *, const void *)) {
    while (root < end / 2) {
        size_t child = root * 2 + 1;
        if (child + 1 < end &&
            compare(element(base, size, child), element(base, size, child + 1)) < 0)
            ++child;
        if (compare(element(base, size, root), element(base, size, child)) >= 0)
            return;
        swap_elements(element(base, size, root), element(base, size, child), size);
        root = child;
    }
}

void qsort(void *base, size_t count, size_t size,
           int (*compare)(const void *, const void *)) {
    size_t start;
    size_t end;
    if (base == 0 || compare == 0 || size == 0 || count < 2) return;
    start = count / 2;
    while (start != 0) sift_down(base, --start, count, size, compare);
    end = count;
    while (end > 1) {
        --end;
        swap_elements(element(base, size, 0), element(base, size, end), size);
        sift_down(base, 0, end, size, compare);
    }
}

void *bsearch(const void *key, const void *base, size_t count, size_t size,
              int (*compare)(const void *, const void *)) {
    size_t low = 0;
    size_t high = count;
    if (key == 0 || base == 0 || compare == 0 || size == 0) return 0;
    while (low < high) {
        size_t middle = low + (high - low) / 2;
        const void *candidate = element((void *)base, size, middle);
        int relation = compare(key, candidate);
        if (relation == 0) return (void *)candidate;
        if (relation < 0) high = middle;
        else low = middle + 1;
    }
    return 0;
}
