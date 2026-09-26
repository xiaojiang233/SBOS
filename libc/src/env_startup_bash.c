#include <unistd.h>

static char *empty_environment[1] = {0};
char **environ = empty_environment;

void __sbos_init_environment(char **initial) {
    environ = initial != 0 ? initial : empty_environment;
}
