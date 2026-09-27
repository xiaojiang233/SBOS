#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

int main(void) {
    int listener = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    struct sockaddr_in local;
    int connection;
    char buffer[64];
    ssize_t count;

    if (listener < 0) { puts("tcp-listen-probe: socket failed"); return 1; }
    memset(&local, 0, sizeof(local));
    local.sin_family = AF_INET;
    local.sin_port = htons(6000);
    local.sin_addr.s_addr = htonl(INADDR_ANY);
    if (bind(listener, (const struct sockaddr *)&local, sizeof(local)) != 0 ||
        listen(listener, 1) != 0) {
        puts("tcp-listen-probe: bind/listen failed");
        close(listener);
        return 2;
    }
    puts("TCP listener ready on port 6000");
    connection = accept(listener, 0, 0);
    if (connection < 0) {
        puts("tcp-listen-probe: accept failed");
        close(listener);
        return 3;
    }
    count = read(connection, buffer, sizeof(buffer));
    if (count <= 0) {
        puts("tcp-listen-probe: read failed");
        close(connection);
        close(listener);
        return 4;
    }
    if (write(connection, buffer, (size_t)count) != count) {
        puts("tcp-listen-probe: write failed");
        close(connection);
        close(listener);
        return 5;
    }
    printf("TCP accepted and echoed %ld bytes\n", (long)count);
    close(connection);
    close(listener);
    return 0;
}
