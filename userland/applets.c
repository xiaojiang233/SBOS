#include <dirent.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <unistd.h>

static const char *basename_of(const char *path) {
    const char *base = path;
    if (path == 0) return "";
    for (; *path; ++path) if (*path == '/') base = path + 1;
    return base;
}

static int command_ls(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : ".";
    DIR *directory = opendir(path);
    struct dirent *entry;
    if (directory == 0) {
        fprintf(stderr, "ls: cannot open %s\n", path);
        return 1;
    }
    while ((entry = readdir(directory)) != 0) {
        puts(entry->d_name);
    }
    (void)closedir(directory);
    return 0;
}

static int command_cat(int argc, char **argv) {
    char buffer[1024];
    int index;
    if (argc < 2) {
        fputs("cat: expected a file path\n", stderr);
        return 2;
    }
    for (index = 1; index < argc; ++index) {
        int fd = open(argv[index], O_RDONLY);
        ssize_t count;
        if (fd < 0) {
            fprintf(stderr, "cat: cannot open %s\n", argv[index]);
            return 1;
        }
        while ((count = read(fd, buffer, sizeof(buffer))) > 0) {
            if (write(STDOUT_FILENO, buffer, (size_t)count) != count) {
                (void)close(fd);
                return 1;
            }
        }
        (void)close(fd);
        if (count < 0) return 1;
    }
    return 0;
}

static int command_clear(void) {
    static const char sequence[] = "\033[2J\033[H";
    return write(STDOUT_FILENO, sequence, sizeof(sequence) - 1) < 0;
}

static int command_id(void) {
    printf("uid=%u gid=%u pid=%d\n", (unsigned)getuid(), (unsigned)getgid(), (int)getpid());
    return 0;
}

static int command_mv(int argc, char **argv) {
    if (argc != 3) {
        fputs("usage: mv SOURCE DESTINATION\n", stderr);
        return 2;
    }
    if (rename(argv[1], argv[2]) < 0) {
        fprintf(stderr, "mv: cannot move %s to %s\n", argv[1], argv[2]);
        return 1;
    }
    return 0;
}

int main(int argc, char **argv) {
    const char *command = basename_of(argc > 0 ? argv[0] : "");
    if (strcmp(command, "ls") == 0) return command_ls(argc, argv);
    if (strcmp(command, "cat") == 0) return command_cat(argc, argv);
    if (strcmp(command, "clear") == 0) return command_clear();
    if (strcmp(command, "id") == 0) return command_id();
    if (strcmp(command, "mv") == 0) return command_mv(argc, argv);
    fputs("SBOS applet: invoke as ls, cat, clear, id, or mv\n", stderr);
    return 127;
}
