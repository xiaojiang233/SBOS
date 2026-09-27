#include <sbos/native.h>
#include <stdio.h>

int main(void) {
    struct sbos_display_info display;
    if (sbos_display_get_info(&display) != (int64_t)sizeof(display) ||
        display.width < 240 || display.height < 160) {
        puts("desktop-demo: framebuffer unavailable or too small");
        return 1;
    }

    if (sbos_display_fill_rect(0, 0, display.width, display.height, 0x00121a2b) != 0 ||
        sbos_display_fill_rect(0, 0, display.width, 34, 0x00212d42) != 0 ||
        sbos_display_fill_rect(28, 60, display.width - 56, display.height - 92,
                               0x00e7edf5) != 0 ||
        sbos_display_fill_rect(28, 60, display.width - 56, 30, 0x003a587a) != 0 ||
        sbos_display_fill_rect(48, 112, display.width / 3, 12, 0x0048668a) != 0 ||
        sbos_display_fill_rect(48, 136, display.width / 2, 8, 0x00aebdce) != 0 ||
        sbos_display_fill_rect(48, 158, display.width / 2 - 20, 8, 0x00aebdce) != 0 ||
        sbos_display_fill_rect(48, 198, 112, 30, 0x003c77b3) != 0) {
        puts("desktop-demo: drawing request failed");
        return 2;
    }

    printf("GOP desktop surface rendered: %ux%u, pixel format %u\n",
           display.width, display.height, display.pixel_format);
    return 0;
}
