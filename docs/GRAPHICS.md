# Graphics and X11 roadmap

## Current framebuffer surface

The UEFI loader passes GOP geometry and pixel format to the kernel. The
`driver-framebuffer` backend now exposes a small, capability-checked Ring 3
surface API:

- query width, height, and pixel format;
- fill a clipped rectangle with canonical `0x00RRGGBB` color;
- blit a bounded pixel rectangle from validated user memory.

`desktop-demo` exercises these calls and paints a simple desktop panel. The
QEMU smoke transcript reports the 1280×800 GOP surface and successful draw
requests. The API caps each blit at one 4 KiB page; it is a bootstrap interface,
not a general compositor or shared scanout mapping.

## Input groundwork

The optional `driver-ps2-mouse` enables the i8042 auxiliary port, decodes
three-byte PS/2 packets from the PIT polling path, bounds cursor coordinates to
the display, and queues relative motion/button events. `MouseReadEvent` (Native
syscall 78) returns typed events to a caller with the `INPUT` capability.
`mouse-probe` confirms the driver initializes and the nonblocking event query
works when the queue is empty. The reference profile enables this feature.

## X server status

`/Applications/xserver` is now a Ring 3 X11 core-protocol prototype. It is a
separate C process and uses the TCP listener and GOP display APIs. The current
subset accepts little- or big-endian X11 11.0 setup with empty authorization,
reports one 24-bit TrueColor screen, and handles basic window/GC creation,
map/unmap, configure, `PolyFillRectangle`, `ClearArea`, `GetGeometry`,
`GetWindowAttributes`, `QueryTree`, `GetInputFocus`, `InternAtom`, and
`QueryExtension` requests. Pointer motion and button events are emitted for
windows that select the corresponding core event masks. Unknown requests close
the client connection.

The QEMU X11 smoke ran `xserver -noauth -once` and connected a small external
wire-protocol client over a host-forward bound to `127.0.0.1`. Setup, window
creation, mapping, rectangle drawing, and a geometry reply all passed. The
probe source is `tools/x11-probe.py`; the serial transcript is
`build/x11-smoke-transcript.log`.

This is not a full X.Org server. Authentication is intentionally not
implemented: `-noauth` must be supplied, and the QEMU test forwards only to the
host loopback address. It currently handles one client at a time, does not
implement a window manager/compositor, pixmaps, fonts, properties, selections,
most core requests, X extensions, or keyboard events to X clients. Pointer event
delivery is implemented but has not yet been verified with injected mouse
motion. The display demo remains available independently.

The protocol subset follows the [X Window System Protocol specification](https://www.x.org/releases/current/doc/xproto/x11protocol.pdf).
