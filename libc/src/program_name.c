static char *initial_program_name;

void __sbos_init_program_name(char *initial) {
    initial_program_name = initial;
}

const char *__sbos_program_short_name(void) {
    const char *name = initial_program_name;
    const char *component = name;
    if (name == 0 || name[0] == '\0') return "?";
    for (; *name != '\0'; ++name)
        if (*name == '/') component = name + 1;
    return component[0] != '\0' ? component : "?";
}
