/*
 * Experimental SBOS X11 core-protocol server.
 *
 * This deliberately small server implements an unauthenticated development
 * mode and a limited request set. Do not expose it beyond a private QEMU
 * host-forward. Window and drawing state stays in this Ring 3 process.
 */
#include <netinet/in.h>
#include <errno.h>
#include <fcntl.h>
#include <sbos/native.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

#define X11_PORT 6000
#define X11_MAX_REQUEST 4096
#define X11_WINDOW_COUNT 64
#define X11_GC_COUNT 64
#define X11_ATOM_COUNT 64
#define X11_RESOURCE_BASE 0x01000000u

struct XWindow {
    uint32_t id;
    uint32_t parent;
    int16_t x;
    int16_t y;
    uint16_t width;
    uint16_t height;
    uint32_t background;
    uint32_t event_mask;
    uint8_t mapped;
};

struct XGc {
    uint32_t id;
    uint32_t foreground;
    uint32_t background;
    uint8_t live;
};

struct XAtom {
    uint32_t id;
    uint16_t length;
    char name[128];
};

static struct XWindow windows[X11_WINDOW_COUNT];
static struct XGc gcs[X11_GC_COUNT];
static struct XAtom atoms[X11_ATOM_COUNT];
static uint32_t next_atom = 100;
static uint16_t sequence_number;
static uint8_t client_little_endian;
static struct sbos_display_info display_info;
static int32_t pointer_x;
static int32_t pointer_y;
static uint8_t pointer_buttons;
static uint32_t input_focus = 1;
static int keyboard_input_claimed;

static uint16_t get16(const unsigned char *p) {
    return client_little_endian ? (uint16_t)(p[0] | ((uint16_t)p[1] << 8))
                                : (uint16_t)(((uint16_t)p[0] << 8) | p[1]);
}

static uint32_t get32(const unsigned char *p) {
    if (client_little_endian)
        return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
               ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
    return ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) |
           ((uint32_t)p[2] << 8) | (uint32_t)p[3];
}

static void put16(unsigned char *p, uint16_t value) {
    if (client_little_endian) {
        p[0] = (unsigned char)value;
        p[1] = (unsigned char)(value >> 8);
    } else {
        p[0] = (unsigned char)(value >> 8);
        p[1] = (unsigned char)value;
    }
}

static void put32(unsigned char *p, uint32_t value) {
    if (client_little_endian) {
        p[0] = (unsigned char)value;
        p[1] = (unsigned char)(value >> 8);
        p[2] = (unsigned char)(value >> 16);
        p[3] = (unsigned char)(value >> 24);
    } else {
        p[0] = (unsigned char)(value >> 24);
        p[1] = (unsigned char)(value >> 16);
        p[2] = (unsigned char)(value >> 8);
        p[3] = (unsigned char)value;
    }
}

static int read_exact(int fd, void *buffer, size_t length) {
    size_t done = 0;
    while (done < length) {
        ssize_t count = read(fd, (unsigned char *)buffer + done, length - done);
        if (count <= 0) return 0;
        done += (size_t)count;
    }
    return 1;
}

static int write_exact(int fd, const void *buffer, size_t length) {
    size_t done = 0;
    while (done < length) {
        ssize_t count = write(fd, (const unsigned char *)buffer + done, length - done);
        if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK)) {
            struct timespec pause = {0, 1000000L};
            (void)nanosleep(&pause, 0);
            continue;
        }
        if (count <= 0) return 0;
        done += (size_t)count;
    }
    return 1;
}

static size_t pad4(size_t length) { return (length + 3u) & ~(size_t)3u; }

static void make_setup_reply(unsigned char *reply, size_t *reply_length) {
    static const char vendor[] = "SBOS";
    size_t at = 8;
    memset(reply, 0, 256);
    /* ConnectionSetup: global server information. */
    put32(reply + at, 1); at += 4; /* release number */
    put32(reply + at, X11_RESOURCE_BASE); at += 4;
    put32(reply + at, 0x00ffffff); at += 4;
    put32(reply + at, 0); at += 4; /* motion buffer */
    put16(reply + at, sizeof(vendor) - 1); at += 2;
    put16(reply + at, 1024); at += 2; /* max request length in 4-byte units */
    reply[at++] = 1; /* screens */
    reply[at++] = 1; /* pixmap formats */
    reply[at++] = 0; /* image byte order: LSBFirst */
    reply[at++] = 0; /* bitmap bit order */
    reply[at++] = 32;
    reply[at++] = 32;
    reply[at++] = 8;   /* min keycode */
    reply[at++] = 255; /* max keycode */
    at += 4; /* pad */

    memcpy(reply + at, vendor, sizeof(vendor) - 1);
    at += pad4(sizeof(vendor) - 1);

    /* One depth-24 Z pixmap format stored in 32 bits per pixel. */
    reply[at++] = 24;
    reply[at++] = 32;
    reply[at++] = 32;
    reply[at++] = 0;

    /* One screen. */
    put32(reply + at, 1); at += 4; /* root window */
    put32(reply + at, 2); at += 4; /* default colormap */
    put32(reply + at, 0x00ffffff); at += 4; /* white pixel */
    put32(reply + at, 0); at += 4; /* black pixel */
    put32(reply + at, 0); at += 4; /* current input masks */
    put16(reply + at, (uint16_t)display_info.width); at += 2;
    put16(reply + at, (uint16_t)display_info.height); at += 2;
    put16(reply + at, 320); at += 2;
    put16(reply + at, 200); at += 2;
    put16(reply + at, 1); at += 2; /* min installed maps */
    put16(reply + at, 1); at += 2; /* max installed maps */
    put32(reply + at, 0x21); at += 4; /* root visual */
    reply[at++] = 0; /* backing stores: never */
    reply[at++] = 0; /* save unders */
    reply[at++] = 24; /* root depth */
    reply[at++] = 1;  /* allowed depths */

    /* One depth and one TrueColor visual. */
    reply[at++] = 24;
    reply[at++] = 0;
    put16(reply + at, 1); at += 2;
    put32(reply + at, 0x21); at += 4;
    reply[at++] = 4; /* TrueColor */
    reply[at++] = 8;
    put16(reply + at, 256); at += 2;
    put32(reply + at, 0x00ff0000); at += 4;
    put32(reply + at, 0x0000ff00); at += 4;
    put32(reply + at, 0x000000ff); at += 4;
    at += 4; /* pad */

    reply[0] = 1; /* success */
    put16(reply + 2, 11);
    put16(reply + 4, 0);
    put16(reply + 6, (uint16_t)((at - 8) / 4));
    *reply_length = at;
}

static int setup_client(int fd, int no_auth) {
    unsigned char request[12];
    unsigned char reply[256];
    unsigned char discard[128];
    uint16_t auth_name_length, auth_data_length;
    size_t reply_length;
    if (!read_exact(fd, request, sizeof(request))) return 0;
    if (request[0] == 'l') client_little_endian = 1;
    else if (request[0] == 'B') client_little_endian = 0;
    else return 0;
    auth_name_length = get16(request + 6);
    auth_data_length = get16(request + 8);
    if (auth_name_length > sizeof(discard) || auth_data_length > sizeof(discard)) return 0;
    if (auth_name_length && !read_exact(fd, discard, pad4(auth_name_length))) return 0;
    if (auth_data_length && !read_exact(fd, discard, pad4(auth_data_length))) return 0;
    if (!no_auth || auth_name_length || auth_data_length || get16(request + 2) != 11) {
        static const char reason[] = "SBOS xserver requires explicit development -noauth mode";
        size_t padded = pad4(sizeof(reason) - 1);
        memset(reply, 0, 8 + padded);
        reply[0] = 0;
        reply[1] = (unsigned char)(sizeof(reason) - 1);
        put16(reply + 2, 11);
        put16(reply + 4, 0);
        put16(reply + 6, (uint16_t)(padded / 4));
        memcpy(reply + 8, reason, sizeof(reason) - 1);
        return write_exact(fd, reply, 8 + padded);
    }
    make_setup_reply(reply, &reply_length);
    return write_exact(fd, reply, reply_length);
}

static struct XWindow *find_window(uint32_t id) {
    unsigned i;
    if (id == 1) return &windows[0];
    for (i = 1; i < X11_WINDOW_COUNT; ++i)
        if (windows[i].id == id) return &windows[i];
    return 0;
}

static struct XWindow *new_window(uint32_t id) {
    unsigned i;
    for (i = 1; i < X11_WINDOW_COUNT; ++i) {
        if (windows[i].id == 0) {
            memset(&windows[i], 0, sizeof(windows[i]));
            windows[i].id = id;
            windows[i].background = 0x00e7edf5;
            return &windows[i];
        }
    }
    return 0;
}

static struct XGc *find_gc(uint32_t id) {
    unsigned i;
    for (i = 0; i < X11_GC_COUNT; ++i)
        if (gcs[i].live && gcs[i].id == id) return &gcs[i];
    return 0;
}

static struct XGc *new_gc(uint32_t id) {
    unsigned i;
    for (i = 0; i < X11_GC_COUNT; ++i) {
        if (!gcs[i].live) {
            gcs[i].id = id;
            gcs[i].foreground = 0;
            gcs[i].background = 0x00ffffff;
            gcs[i].live = 1;
            return &gcs[i];
        }
    }
    return 0;
}

static void send_map_notify(int fd, uint32_t event_window, uint32_t window_id) {
    unsigned char event[32] = {0};
    event[0] = 19; /* MapNotify */
    put16(event + 2, sequence_number);
    put32(event + 4, event_window);
    put32(event + 8, window_id);
    (void)write_exact(fd, event, sizeof(event));
}

static void send_expose(int fd, struct XWindow *window) {
    unsigned char event[32] = {0};
    event[0] = 12; /* Expose */
    put16(event + 2, sequence_number);
    put32(event + 4, window->id);
    put16(event + 12, window->width);
    put16(event + 14, window->height);
    put16(event + 16, 0); /* final exposure in this sequence */
    (void)write_exact(fd, event, sizeof(event));
}

static void send_configure_notify(int fd, struct XWindow *window) {
    unsigned char event[32] = {0};
    event[0] = 22; /* ConfigureNotify */
    put16(event + 2, sequence_number);
    put32(event + 4, window->id);
    put32(event + 8, window->id);
    put32(event + 12, 0); /* above sibling */
    put16(event + 16, (uint16_t)window->x);
    put16(event + 18, (uint16_t)window->y);
    put16(event + 20, window->width);
    put16(event + 22, window->height);
    put16(event + 24, 0); /* border width */
    event[26] = 0; /* not override redirect */
    (void)write_exact(fd, event, sizeof(event));
}

static uint32_t value_for_mask(const unsigned char *request, size_t value_offset,
                               uint32_t mask, unsigned wanted_bit) {
    unsigned bit;
    unsigned index = 0;
    for (bit = 0; bit < 32; ++bit) {
        if ((mask & (1u << bit)) != 0) {
            uint32_t value = get32(request + value_offset + index * 4);
            if (bit == wanted_bit) return value;
            ++index;
        }
    }
    return 0;
}

static void send_reply_header(int fd, unsigned char *reply, uint8_t type,
                              uint16_t sequence, uint32_t length_words) {
    memset(reply, 0, 64);
    reply[0] = type;
    put16(reply + 2, sequence);
    put32(reply + 4, length_words);
    (void)fd;
}

static void draw_window_background(struct XWindow *window) {
    int x = window->x;
    int y = window->y;
    if (window->id == 1) { x = 0; y = 0; }
    if (x < 0) { window->width = (uint16_t)((int)window->width + x); x = 0; }
    if (y < 0) { window->height = (uint16_t)((int)window->height + y); y = 0; }
    (void)sbos_display_fill_rect((uint32_t)x, (uint32_t)y, window->width,
                                 window->height, window->background);
}

static struct XWindow *window_under_pointer(void) {
    int i;
    for (i = X11_WINDOW_COUNT - 1; i > 0; --i) {
        struct XWindow *window = &windows[i];
        if (window->mapped && pointer_x >= window->x && pointer_y >= window->y &&
            pointer_x < window->x + window->width && pointer_y < window->y + window->height)
            return window;
    }
    return &windows[0];
}

static void send_pointer_event(int fd, uint8_t type, uint8_t detail,
                               struct XWindow *window, uint8_t previous_buttons) {
    unsigned char event[32] = {0};
    struct timespec now;
    uint32_t time_ms = 0;
    if (clock_gettime(CLOCK_MONOTONIC, &now) == 0)
        time_ms = (uint32_t)((uint64_t)now.tv_sec * 1000 + (uint64_t)now.tv_nsec / 1000000);
    event[0] = type;
    event[1] = detail;
    put16(event + 2, sequence_number);
    put32(event + 4, time_ms);
    put32(event + 8, 1); /* root */
    put32(event + 12, window->id);
    put32(event + 16, 0); /* child */
    put16(event + 20, (uint16_t)pointer_x);
    put16(event + 22, (uint16_t)pointer_y);
    put16(event + 24, (uint16_t)(pointer_x - window->x));
    put16(event + 26, (uint16_t)(pointer_y - window->y));
    put16(event + 28,
          ((previous_buttons & 1) ? (1u << 8) : 0) |
          ((previous_buttons & 2) ? (1u << 9) : 0) |
          ((previous_buttons & 4) ? (1u << 10) : 0));
    event[30] = 1; /* same screen */
    (void)write_exact(fd, event, sizeof(event));
}

static void pump_pointer_events(int fd) {
    struct sbos_mouse_event mouse;
    int64_t result;
    while ((result = sbos_mouse_read_event(&mouse, 1)) == 0) {
        struct XWindow *window;
        uint8_t old_buttons = pointer_buttons;
        pointer_x = mouse.x;
        pointer_y = mouse.y;
        pointer_buttons = mouse.buttons;
        window = window_under_pointer();
        if ((mouse.delta_x != 0 || mouse.delta_y != 0) &&
            (window->event_mask & (1u << 6)))
            send_pointer_event(fd, 6, 0, window, old_buttons);
        {
            unsigned button;
            for (button = 0; button < 3; ++button) {
                uint8_t button_mask = (uint8_t)(1u << button);
                uint32_t event_mask = (pointer_buttons & button_mask) ? (1u << 2) : (1u << 3);
                if ((mouse.changed_buttons & button_mask) && (window->event_mask & event_mask))
                    send_pointer_event(fd, (pointer_buttons & button_mask) ? 4 : 5,
                                       (uint8_t)(button + 1), window, old_buttons);
            }
        }
    }
}

static void pump_keyboard_events(int fd) {
    struct sbos_key_event key;
    if (!keyboard_input_claimed) return;
    while (sbos_keyboard_read_event(&key, 1) == 0) {
        struct XWindow *window = find_window(input_focus);
        uint32_t mask = key.pressed ? (1u << 0) : (1u << 1);
        unsigned char event[32] = {0};
        struct timespec now;
        uint32_t time_ms = 0;
        if (!window || !window->mapped || !(window->event_mask & mask)) continue;
        if (clock_gettime(CLOCK_MONOTONIC, &now) == 0)
            time_ms = (uint32_t)((uint64_t)now.tv_sec * 1000 + (uint64_t)now.tv_nsec / 1000000);
        event[0] = key.pressed ? 2 : 3; /* KeyPress / KeyRelease */
        event[1] = key.keycode;
        put16(event + 2, sequence_number);
        put32(event + 4, time_ms);
        put32(event + 8, 1); /* root */
        put32(event + 12, window->id);
        put32(event + 16, 0); /* child */
        put16(event + 20, (uint16_t)pointer_x);
        put16(event + 22, (uint16_t)pointer_y);
        put16(event + 24, (uint16_t)(pointer_x - window->x));
        put16(event + 26, (uint16_t)(pointer_y - window->y));
        put16(event + 28, key.modifiers);
        event[30] = 1; /* same screen */
        (void)write_exact(fd, event, sizeof(event));
    }
}

static int atom_id(const unsigned char *name, uint16_t length, int only_if_exists) {
    unsigned i;
    for (i = 0; i < X11_ATOM_COUNT; ++i)
        if (atoms[i].id && atoms[i].length == length &&
            memcmp(atoms[i].name, name, length) == 0) return (int)atoms[i].id;
    if (only_if_exists) return 0;
    for (i = 0; i < X11_ATOM_COUNT; ++i) {
        if (atoms[i].id == 0 && length < sizeof(atoms[i].name)) {
            atoms[i].id = next_atom++;
            atoms[i].length = length;
            memcpy(atoms[i].name, name, length);
            return (int)atoms[i].id;
        }
    }
    return 0;
}

static int serve_request(int fd, const unsigned char *request, size_t length) {
    unsigned char reply[64];
    uint8_t opcode = request[0];
    uint16_t sequence = ++sequence_number;
    uint32_t id, mask;
    struct XWindow *window;
    struct XGc *gc;
    switch (opcode) {
    case 1: /* CreateWindow */
        if (length < 32) return 0;
        id = get32(request + 4);
        mask = get32(request + 28);
        if (32u + (size_t)__builtin_popcount(mask) * 4 > length) return 0;
        if (!find_window(id) && (window = new_window(id)) != 0) {
            window->parent = get32(request + 8);
            window->x = (int16_t)get16(request + 12);
            window->y = (int16_t)get16(request + 14);
            window->width = get16(request + 16);
            window->height = get16(request + 18);
            if (mask & (1u << 1)) window->background = value_for_mask(request, 32, mask, 1);
            if (mask & (1u << 11)) window->event_mask = value_for_mask(request, 32, mask, 11);
        if (mask & (1u << 11)) window->event_mask = value_for_mask(request, 32, mask, 11);
        }
        return 1;
    case 2: /* ChangeWindowAttributes */
        if (length < 12) return 0;
        window = find_window(get32(request + 4));
        mask = get32(request + 8);
        if (12u + (size_t)__builtin_popcount(mask) * 4 > length) return 0;
        if (window && (mask & (1u << 1)))
            window->background = value_for_mask(request, 12, mask, 1);
        if (window && (mask & (1u << 11)))
            window->event_mask = value_for_mask(request, 12, mask, 11);
        return 1;
    case 3: /* GetWindowAttributes */
        if (length < 8) return 0;
        window = find_window(get32(request + 4));
        if (!window) return 0;
        send_reply_header(fd, reply, 1, sequence, 3);
        reply[1] = 0; /* backing store */
        put32(reply + 8, 0x21); /* visual */
        put16(reply + 12, 1); /* InputOutput */
        reply[14] = 0; reply[15] = 0; /* gravities */
        put32(reply + 16, 0xffffffff);
        put32(reply + 20, window->background);
        reply[24] = 0; reply[25] = 1; reply[26] = window->mapped; reply[27] = 0;
        put32(reply + 28, 2); /* default colormap */
        return write_exact(fd, reply, 44);
    case 4: /* DestroyWindow */
        if (length < 8) return 0;
        window = find_window(get32(request + 4));
        if (window && window->id != 1) memset(window, 0, sizeof(*window));
        return 1;
    case 8: /* MapWindow */
        if (length < 8) return 0;
        window = find_window(get32(request + 4));
        if (window) {
            window->mapped = 1;
            draw_window_background(window);
            if (window->event_mask & (1u << 17)) send_map_notify(fd, window->id, window->id);
            if (window->event_mask & (1u << 15)) send_expose(fd, window);
            {
                struct XWindow *parent = find_window(window->parent);
                if (parent && (parent->event_mask & (1u << 19)))
                    send_map_notify(fd, parent->id, window->id);
            }
        }
        return 1;
    case 10: /* UnmapWindow */
        if (length < 8) return 0;
        window = find_window(get32(request + 4));
        if (window && window->id != 1) window->mapped = 0;
        return 1;
    case 12: /* ConfigureWindow: support x/y/width/height */
        if (length < 12) return 0;
        window = find_window(get32(request + 4));
        mask = get16(request + 8);
        if (!window) return 1;
        {
            unsigned index = 0;
            unsigned bit;
            for (bit = 0; bit < 7; ++bit) {
                uint32_t value;
                if ((mask & (1u << bit)) == 0) continue;
                value = get32(request + 12 + index++ * 4);
                if (bit == 0) window->x = (int16_t)value;
                else if (bit == 1) window->y = (int16_t)value;
                else if (bit == 2) window->width = (uint16_t)value;
                else if (bit == 3) window->height = (uint16_t)value;
            }
            if (window->mapped) {
                draw_window_background(window);
                if (window->event_mask & (1u << 17)) send_configure_notify(fd, window);
            }
        }
        return 1;
    case 14: /* GetGeometry */
        if (length < 8) return 0;
        window = find_window(get32(request + 4));
        if (!window) return 0;
        send_reply_header(fd, reply, 1, sequence, 0);
        reply[1] = 24;
        put32(reply + 8, 1);
        put16(reply + 12, (uint16_t)window->x);
        put16(reply + 14, (uint16_t)window->y);
        put16(reply + 16, window->width);
        put16(reply + 18, window->height);
        put16(reply + 20, 0);
        return write_exact(fd, reply, 32);
    case 16: /* InternAtom */
        if (length < 8) return 0;
        {
            uint16_t name_length = get16(request + 6);
            int atom;
            if ((size_t)name_length + 8 > length) return 0;
            atom = atom_id(request + 8, name_length, request[1] != 0);
            send_reply_header(fd, reply, 1, sequence, 0);
            put32(reply + 8, (uint32_t)atom);
            return write_exact(fd, reply, 32);
        }
    case 42: /* SetInputFocus */
        if (length < 12) return 0;
        if (get32(request + 4) == 0) input_focus = 1;
        else if (find_window(get32(request + 4))) input_focus = get32(request + 4);
        return 1;
    case 43: /* GetInputFocus */
        send_reply_header(fd, reply, 1, sequence, 0);
        put32(reply + 8, input_focus);
        reply[12] = 0;
        return write_exact(fd, reply, 32);
    case 55: /* CreateGC */
        if (length < 16) return 0;
        gc = new_gc(get32(request + 4));
        if (!gc) return 0;
        mask = get32(request + 12);
        if (mask & (1u << 2)) gc->foreground = value_for_mask(request, 16, mask, 2);
        if (mask & (1u << 3)) gc->background = value_for_mask(request, 16, mask, 3);
        return 1;
    case 56: /* ChangeGC */
        if (length < 12) return 0;
        gc = find_gc(get32(request + 4));
        mask = get32(request + 8);
        if (!gc) return 1;
        if (mask & (1u << 2)) gc->foreground = value_for_mask(request, 12, mask, 2);
        if (mask & (1u << 3)) gc->background = value_for_mask(request, 12, mask, 3);
        return 1;
    case 60: /* FreeGC */
        if (length < 8) return 0;
        gc = find_gc(get32(request + 4));
        if (gc) memset(gc, 0, sizeof(*gc));
        return 1;
    case 61: /* ClearArea */
        if (length < 20) return 0;
        window = find_window(get32(request + 4));
        if (window && window->mapped) {
            uint16_t x = get16(request + 8), y = get16(request + 10);
            uint16_t width = get16(request + 12), height = get16(request + 14);
            (void)sbos_display_fill_rect((uint32_t)(window->x + (int16_t)x),
                (uint32_t)(window->y + (int16_t)y), width, height, window->background);
        }
        return 1;
    case 70: /* PolyFillRectangle */
        if (length < 20) return 0;
        window = find_window(get32(request + 4));
        gc = find_gc(get32(request + 8));
        if (!window || !gc) return 1;
        {
            size_t offset;
            for (offset = 12; offset + 8 <= length; offset += 8) {
                uint32_t x = (uint32_t)(window->x + (int16_t)get16(request + offset));
                uint32_t y = (uint32_t)(window->y + (int16_t)get16(request + offset + 2));
                uint32_t width = get16(request + offset + 4);
                uint32_t height = get16(request + offset + 6);
                (void)sbos_display_fill_rect(x, y, width, height, gc->foreground);
            }
        }
        return 1;
    case 98: /* QueryExtension: no extensions are advertised yet. */
        send_reply_header(fd, reply, 1, sequence, 0);
        reply[1] = 0;
        return write_exact(fd, reply, 32);
    case 127: /* NoOperation */
        return 1;
    default:
        return 0;
    }
}

static int serve_client(int fd, int no_auth) {
    unsigned char request[4096];
    size_t used = 0;
    int flags;
    unsigned i;
    for (i = 1; i < X11_WINDOW_COUNT; ++i) memset(&windows[i], 0, sizeof(windows[i]));
    memset(gcs, 0, sizeof(gcs));
    memset(atoms, 0, sizeof(atoms));
    next_atom = 100;
    input_focus = 1;
    if (!setup_client(fd, no_auth)) return 0;
    flags = fcntl(fd, F_GETFL);
    if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) return 0;
    sequence_number = 0;
    for (;;) {
        ssize_t count;
        pump_pointer_events(fd);
        pump_keyboard_events(fd);
        if (used >= 4) {
            uint16_t words = get16(request + 2);
            size_t bytes;
            if (words == 0 || words > (X11_MAX_REQUEST / 4)) return 0;
            bytes = (size_t)words * 4;
            if (used >= bytes) {
                if (!serve_request(fd, request, bytes)) return 0;
                used -= bytes;
                if (used != 0) memmove(request, request + bytes, used);
                continue;
            }
        }
        if (used == sizeof(request)) return 0;
        count = read(fd, request + used, sizeof(request) - used);
        if (count > 0) { used += (size_t)count; continue; }
        if (count == 0) return 1;
        if (errno != EAGAIN && errno != EWOULDBLOCK) return 0;
        {
            struct timespec pause = {0, 10000000L};
            (void)nanosleep(&pause, 0);
        }
    }
}

int main(int argc, char **argv) {
    int no_auth = 0;
    int once = 0;
    int listener;
    struct sockaddr_in local;
    unsigned i;
    for (i = 1; i < (unsigned)argc; ++i)
        if (strcmp(argv[i], "-noauth") == 0) no_auth = 1;
        else if (strcmp(argv[i], "-once") == 0) once = 1;
    if (!no_auth) {
        puts("xserver: only explicit -noauth development mode is available");
        return 2;
    }
    if (sbos_display_get_info(&display_info) < 0 || display_info.width == 0 || display_info.height == 0) {
        puts("xserver: no GOP surface");
        return 1;
    }
    keyboard_input_claimed = sbos_keyboard_claim_input(1) == 0;
    if (!keyboard_input_claimed)
        puts("xserver: keyboard unavailable; continuing with pointer input only");
    memset(windows, 0, sizeof(windows));
    memset(gcs, 0, sizeof(gcs));
    memset(atoms, 0, sizeof(atoms));
    windows[0].id = 1;
    windows[0].width = (uint16_t)display_info.width;
    windows[0].height = (uint16_t)display_info.height;
    windows[0].background = 0x00121a2b;
    windows[0].mapped = 1;
    (void)sbos_display_fill_rect(0, 0, display_info.width, display_info.height, windows[0].background);

    listener = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (listener < 0) { puts("xserver: socket failed"); return 3; }
    memset(&local, 0, sizeof(local));
    local.sin_family = AF_INET;
    local.sin_port = htons(X11_PORT);
    local.sin_addr.s_addr = htonl(INADDR_ANY);
    if (bind(listener, (const struct sockaddr *)&local, sizeof(local)) != 0 || listen(listener, 1) != 0) {
        puts("xserver: bind/listen failed");
        close(listener);
        return 4;
    }
    puts("SBOS minimal X11 server listening on TCP port 6000 (development -noauth mode)");
    for (;;) {
        int client = accept(listener, 0, 0);
        if (client < 0) continue;
        (void)serve_client(client, no_auth);
        close(client);
        if (once) break;
    }
    close(listener);
    return 0;
}
