#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <sys/wait.h>
#include <unistd.h>


int main(int argc, char **argv) {
    int descriptors[2];
    struct stat status;
    char received[32] = {0};
    const char message[] = "fork+pipe";
    char *exec_arguments[] = {"posix-probe", "--exec-target", 0};
    char *exec_environment[] = {"SBOS_EXEC_TARGET=1", 0};
    pid_t child;
    int wait_status = 0;
    ssize_t amount;
    int file;
    struct termios terminal, raw_terminal;
    struct winsize window;

    printf("POSIX probe: pid=%d ppid=%d\n", (int)getpid(), (int)getppid());

    if (argc == 2 && strcmp(argv[1], "--exec-target") == 0) {
        if (getenv("SBOS_EXEC_TARGET") == 0) {
            fprintf(stderr, "POSIX probe: execve environment was lost\n");
            return 10;
        }
        puts("execve: argv, envp, and address-space replacement passed");
        return 0;
    }

    if (getenv("HOME") == 0 || strcmp(getenv("HOME"), "/Users/Root") != 0) {
        fprintf(stderr, "POSIX probe: initial HOME environment is missing\n");
        return 13;
    }

    if (tcgetattr(STDIN_FILENO, &terminal) < 0 ||
        ioctl(STDIN_FILENO, TIOCGWINSZ, &window) < 0) {
        perror("POSIX probe: terminal query");
        return 7;
    }
    raw_terminal = terminal;
    raw_terminal.c_lflag &= ~(ICANON | ECHO);
    raw_terminal.c_cc[VMIN] = 1;
    raw_terminal.c_cc[VTIME] = 0;
    if (tcsetattr(STDIN_FILENO, TCSANOW, &raw_terminal) < 0 ||
        tcsetattr(STDIN_FILENO, TCSANOW, &terminal) < 0) {
        perror("POSIX probe: terminal mode");
        return 8;
    }
    printf("tty=%ux%u mode-change=ok\n", (unsigned)window.ws_col,
           (unsigned)window.ws_row);

    file = open("/System/Readme.txt", O_RDONLY);
    if (file < 0 || fstat(file, &status) < 0 || !S_ISREG(status.st_mode)) {
        perror("POSIX probe: open/fstat");
        return 1;
    }
    printf("stat size=%lld inode=%llu\n",
           (long long)status.st_size, (unsigned long long)status.st_ino);
    close(file);

    {
        const char *source = "/Users/Root/.rename-source";
        const char *destination = "/Users/Root/.rename-destination";
        const char before[] = "old-name";
        (void)unlink(source);
        (void)unlink(destination);
        file = open(source, O_WRONLY | O_CREAT | O_TRUNC, 0600);
        if (file < 0 || write(file, before, sizeof(before)) != (ssize_t)sizeof(before) ||
            close(file) < 0 || rename(source, destination) < 0 ||
            stat(destination, &status) < 0 || stat(source, &status) == 0) {
            perror("POSIX probe: rename");
            return 11;
        }
        file = open(destination, O_RDONLY);
        memset(received, 0, sizeof(received));
        if (file < 0 || read(file, received, sizeof(before)) != (ssize_t)sizeof(before) ||
            memcmp(received, before, sizeof(before)) != 0) {
            perror("POSIX probe: renamed file contents");
            return 12;
        }
        close(file);
        puts("rename: node move and source lookup passed");
    }

    if (pipe(descriptors) < 0) {
        perror("POSIX probe: pipe");
        return 2;
    }
    child = fork();
    if (child < 0) {
        perror("POSIX probe: fork");
        close(descriptors[0]);
        close(descriptors[1]);
        return 3;
    }
    if (child == 0) {
        close(descriptors[0]);
        if (write(descriptors[1], message, sizeof(message) - 1) < 0)
            _exit(4);
        close(descriptors[1]);
        _exit(23);
    }

    close(descriptors[1]);
    amount = read(descriptors[0], received, sizeof(received));
    close(descriptors[0]);
    if (waitpid(child, &wait_status, 0) != child) {
        perror("POSIX probe: waitpid");
        return 5;
    }
    if (amount != (ssize_t)(sizeof(message) - 1) ||
        memcmp(received, message, sizeof(message) - 1) != 0 ||
        !WIFEXITED(wait_status) || WEXITSTATUS(wait_status) != 23) {
        fprintf(stderr, "POSIX probe: bad pipe or child status (%d)\n", wait_status);
        return 6;
    }
    printf("pipe=%s child=%d status=%d\n", received, (int)child,
           WEXITSTATUS(wait_status));

    puts("POSIX probe: fork, waitpid, descriptors and pipe passed");
    execve("/Applications/posix-probe", exec_arguments, exec_environment);
    perror("POSIX probe: execve");
    return 9;
}
