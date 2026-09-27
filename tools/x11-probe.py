#!/usr/bin/env python3
"""Exercise the SBOS development X11 setup and basic window drawing subset."""
import socket
import struct
import sys


def exact(sock, length):
    data = bytearray()
    while len(data) < length:
        chunk = sock.recv(length - len(data))
        if not chunk:
            raise RuntimeError("X server closed the connection early")
        data.extend(chunk)
    return bytes(data)


def request(opcode, data, body):
    total = 4 + len(body)
    if total % 4:
        raise ValueError("request body must be four-byte aligned")
    return struct.pack("<BBH", opcode, data, total // 4) + body


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 16000
    with socket.create_connection(("127.0.0.1", port), timeout=5) as sock:
        # X11 setup: little-endian, protocol 11.0, empty authorization.
        sock.sendall(b"l\0" + struct.pack("<HHHHH", 11, 0, 0, 0, 0))
        prefix = exact(sock, 8)
        if prefix[0] != 1:
            reason = exact(sock, struct.unpack_from("<H", prefix, 6)[0] * 4)
            raise RuntimeError("X11 setup failed: " + reason.decode("ascii", "replace"))
        body = exact(sock, struct.unpack_from("<H", prefix, 6)[0] * 4)
        if len(body) < 108:
            raise RuntimeError(f"short X11 setup body: {len(body)}")
        root = struct.unpack_from("<I", body, 40)[0]
        resource_base = struct.unpack_from("<I", body, 4)[0]
        width, height = struct.unpack_from("<HH", body, 60)
        window = resource_base | 1
        gc = resource_base | 2

        create_window = request(
            1, 24,
            struct.pack("<IIhhHHHHIIII", window, root, 32, 48,
                        240, 140, 0, 1, 0, (1 << 1) | (1 << 11),
                        0x003A77B3, (1 << 15) | (1 << 17)),
        )
        create_gc = request(55, 0, struct.pack("<IIII", gc, window, 1 << 2, 0x00CC3355))
        map_window = request(8, 0, struct.pack("<I", window))
        fill_rectangle = request(70, 0, struct.pack("<IIhhHH", window, gc, 12, 14, 80, 36))
        get_geometry = request(14, 0, struct.pack("<I", window))
        sock.sendall(create_window + create_gc + map_window + fill_rectangle + get_geometry)
        seen_events = set()
        while True:
            reply = exact(sock, 32)
            if reply[0] == 1:
                break
            seen_events.add(reply[0] & 0x7f)
        if not {12, 19}.issubset(seen_events):
            raise RuntimeError(f"missing MapNotify/Expose events: {seen_events}")
        if reply[0] != 1:
            raise RuntimeError(f"GetGeometry reply type was {reply[0]}")
        geometry = struct.unpack_from("<hhHH", reply, 12)
        if geometry != (32, 48, 240, 140):
            raise RuntimeError(f"unexpected X window geometry: {geometry}")
        print(f"X11 setup and drawing requests passed ({width}x{height} root, window {geometry})")


if __name__ == "__main__":
    main()
