#include <fcntl.h>
#include <sbos/native.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

int main(void) {
    static const char path[] = "/Users/Root/.channel-transfer-probe";
    static const char event_prefix[] = "channel-probe/never-changed";
    static const char event_key[] = "channel-probe/never-changed/ready";
    static const char event_value[] = "ready";
    static const char expected[] = "handle-crossed-process-boundary";
    sbos_handle_t channel[2];
    int64_t event = sbos_config_watch(event_prefix, sizeof(event_prefix) - 1);
    if (event < 0 || sbos_call6(SBOS_HANDLE_WAIT, (uint64_t)event, 2, 0, 0, 0, 0) != -7)
        return 7;
    if (sbos_call6(SBOS_CHANNEL_CREATE, (uint64_t)(uintptr_t)channel, 0, 0, 0, 0, 0) < 0) {
        puts("channel transfer probe: channel creation failed");
        return 1;
    }

    int64_t stale = sbos_call6(SBOS_FILE_OPEN, (uint64_t)(uintptr_t)"/System/Readme.txt",
        sizeof("/System/Readme.txt") - 1, 1, 0, 0, 0);
    if (stale < 0 || sbos_call6(SBOS_HANDLE_CLOSE, (uint64_t)stale, 0, 0, 0, 0, 0) < 0 ||
        sbos_call6(SBOS_FILE_READ, (uint64_t)stale, 0, 1, 0, 0, 0) >= 0)
        return 8;

    pid_t child = fork();
    if (child < 0) return 2;
    if (child == 0) {
        if (sbos_call6(SBOS_HANDLE_WAIT, (uint64_t)event, 50, 0, 0, 0, 0) != 0)
            _exit(9);
        char payload[8];
        sbos_handle_t received[2] = {0};
        size_t received_count = 0;
        int64_t bytes = sbos_channel_receive_handles(channel[1], payload,
            sizeof(payload), received, 2, &received_count);
        if (bytes != 1 || payload[0] != 'F' || received_count != 1)
            _exit(10);
        char contents[64] = {0};
        int64_t read_count = sbos_call6(SBOS_FILE_READ, received[0],
            (uint64_t)(uintptr_t)contents, sizeof(contents), 0, 0, 0);
        (void)sbos_call6(SBOS_HANDLE_CLOSE, received[0], 0, 0, 0, 0, 0);
        if (read_count != (int64_t)(sizeof(expected) - 1) ||
            memcmp(contents, expected, sizeof(expected) - 1) != 0)
            _exit(11);
        _exit(0);
    }

    int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (fd < 0 || write(fd, expected, sizeof(expected) - 1) != (ssize_t)(sizeof(expected) - 1))
        return 3;
    close(fd);
    uint64_t start = (uint64_t)sbos_call6(SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0);
    while ((uint64_t)sbos_call6(SBOS_SCHEDULER_TICKS, 0, 0, 0, 0, 0, 0) == start) {}
    if (sbos_call6(SBOS_CONFIG_SET, (uint64_t)(uintptr_t)event_key,
            sizeof(event_key) - 1, (uint64_t)(uintptr_t)event_value,
            sizeof(event_value) - 1, 0, 0) < 0)
        return 12;
    (void)sbos_call6(SBOS_HANDLE_CLOSE, (uint64_t)event, 0, 0, 0, 0, 0);
    int64_t file = sbos_call6(SBOS_FILE_OPEN, (uint64_t)(uintptr_t)path,
        sizeof(path) - 1, 1, 0, 0, 0);
    if (file < 0) return 4;
    const char marker = 'F';
    sbos_handle_t transfer[] = {(sbos_handle_t)file};
    if (sbos_channel_send_handles(channel[0], &marker, 1, transfer, 1) != 1)
        return 5;
    (void)sbos_call6(SBOS_HANDLE_CLOSE, (uint64_t)file, 0, 0, 0, 0, 0);

    int status = 0;
    if (waitpid(child, &status, 0) != child || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        puts("channel transfer probe: child did not read transferred file");
        return 6;
    }
    puts("channel transfer: child received a non-amplified File handle and read it");
    return 0;
}
