#include <sbos/native.h>
#include <stdio.h>

int main(void) {
    struct sbos_mouse_event event;
    int64_t result = sbos_mouse_read_event(&event, 1);
    if (result == 0) {
        printf("PS/2 mouse event: x=%d y=%d dx=%d dy=%d buttons=%u changed=%u\n",
               event.x, event.y, event.delta_x, event.delta_y,
               (unsigned)event.buttons, (unsigned)event.changed_buttons);
        return 0;
    }
    if (result == -6) {
        puts("PS/2 mouse event queue empty");
        return 0;
    }
    printf("PS/2 mouse event API unavailable: native status=%ld\n", (long)result);
    return 1;
}
