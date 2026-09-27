#include <errno.h>
#include <stdio.h>
#include <sys/select.h>
#include <unistd.h>

int main(void) {
    int descriptors[2];
    fd_set read_set;
    struct timeval timeout = {0, 0};
    char byte;
    int result;
    if (pipe(descriptors) != 0) { puts("select-probe: pipe failed"); return 1; }
    FD_ZERO(&read_set);
    FD_SET(descriptors[0], &read_set);
    result = select(descriptors[0] + 1, &read_set, 0, 0, &timeout);
    if (result != 0) { puts("select-probe: empty pipe was reported ready"); return 2; }
    if (write(descriptors[1], "x", 1) != 1) { puts("select-probe: pipe write failed"); return 3; }
    FD_ZERO(&read_set);
    FD_SET(descriptors[0], &read_set);
    result = select(descriptors[0] + 1, &read_set, 0, 0, &timeout);
    if (result != 1 || !FD_ISSET(descriptors[0], &read_set)) {
        printf("select-probe: ready pipe result=%d errno=%d\n", result, errno);
        return 4;
    }
    if (read(descriptors[0], &byte, 1) != 1 || byte != 'x') {
        puts("select-probe: pipe read failed");
        return 5;
    }
    close(descriptors[0]);
    close(descriptors[1]);
    puts("select-probe: zero-timeout and pipe readiness passed");
    return 0;
}
