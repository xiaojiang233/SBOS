#include <stdint.h>
#include <unistd.h>

int main(void) {
    static const char message[] = "fault-test: triggering a user page fault\n";
    (void)write(STDOUT_FILENO, message, sizeof(message) - 1);
    *(volatile uint64_t *)(uintptr_t)0x0000004000000000ULL = 0x53424f53;
    return 99;
}
