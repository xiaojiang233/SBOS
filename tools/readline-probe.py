#!/usr/bin/env python3
"""Type a command through QEMU's PS/2 keyboard and edit it with Readline."""
import json
import socket
import sys
import time


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 16003
    with socket.create_connection(("127.0.0.1", port), timeout=5) as sock:
        stream = sock.makefile("rb")
        greeting = json.loads(stream.readline())
        if "QMP" not in greeting:
            raise RuntimeError("invalid QMP greeting")

        def command(payload):
            sock.sendall(json.dumps(payload).encode("ascii") + b"\r\n")
            while True:
                reply = json.loads(stream.readline())
                if "error" in reply:
                    raise RuntimeError(f"QMP command failed: {reply['error']}")
                if "return" in reply:
                    return reply["return"]

        command({"execute": "qmp_capabilities"})

        def key(name):
            command({
                "execute": "human-monitor-command",
                "arguments": {"command-line": f"sendkey {name}"},
            })
            # HMP holds each key for 100ms before synthesizing key release.
            time.sleep(0.12)

        for char in "echo nope":
            key("spc" if char == " " else char)
        for _ in range(4):
            key("left")
        for char in "readline-":
            key("minus" if char == "-" else char)
        key("ret")
        time.sleep(1)
        for char in "exit":
            key(char)
        key("ret")

    print("Readline probe injected an in-line edit and clean shell exit")


if __name__ == "__main__":
    main()
