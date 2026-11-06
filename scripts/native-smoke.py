#!/usr/bin/env python3
"""Check the native X11 window and drag distance. Requires libX11/libXtst.

Prefer an isolated X11 display (e.g. Xephyr with a window manager). The test
moves the pointer and restores it after the application exits.
"""
import argparse
import ctypes as C
import ctypes.util
import json
import subprocess
import time

D, W, I, U = C.c_void_p, C.c_ulong, C.c_int, C.c_uint
X = C.CDLL(ctypes.util.find_library("X11"))
T = C.CDLL(ctypes.util.find_library("Xtst"))


class WindowAttributes(C.Structure):
    _fields_ = [("background_pixmap", W), ("background_pixel", W),
                ("border_pixmap", W), ("border_pixel", W),
                ("bit_gravity", I), ("win_gravity", I), ("backing_store", I),
                ("backing_planes", W), ("backing_pixel", W), ("save_under", I),
                ("event_mask", C.c_long), ("do_not_propagate_mask", C.c_long),
                ("override_redirect", I), ("colormap", W), ("cursor", W)]


def bind(lib, name, args, result=I):
    function = getattr(lib, name)
    function.argtypes, function.restype = args, result
    return function


bind(X, "XOpenDisplay", [C.c_char_p], D)
bind(X, "XDefaultRootWindow", [D], W)
bind(X, "XQueryTree", [D, W, C.POINTER(W), C.POINTER(W), C.POINTER(C.POINTER(W)), C.POINTER(U)])
bind(X, "XGetWindowProperty", [D, W, W, C.c_long, C.c_long, I, W, C.POINTER(W), C.POINTER(I), C.POINTER(W), C.POINTER(W), C.POINTER(C.POINTER(C.c_ubyte))])
bind(X, "XInternAtom", [D, C.c_char_p, I], W)
bind(X, "XGetAtomName", [D, W], D)
bind(X, "XFree", [D])
bind(X, "XTranslateCoordinates", [D, W, W, I, I, C.POINTER(I), C.POINTER(I), C.POINTER(W)])
bind(X, "XGetGeometry", [D, W, C.POINTER(W), C.POINTER(I), C.POINTER(I), C.POINTER(U), C.POINTER(U), C.POINTER(U), C.POINTER(U)])
bind(X, "XQueryPointer", [D, W, C.POINTER(W), C.POINTER(W), C.POINTER(I), C.POINTER(I), C.POINTER(I), C.POINTER(I), C.POINTER(U)])
bind(X, "XSync", [D, I])
bind(X, "XCloseDisplay", [D])
bind(X, "XSetInputFocus", [D, W, I, W])
bind(X, "XRaiseWindow", [D, W])
bind(X, "XCreateSimpleWindow", [D, W, I, I, U, U, U, W, W], W)
bind(X, "XChangeWindowAttributes", [D, W, W, C.POINTER(WindowAttributes)])
bind(X, "XSelectInput", [D, W, C.c_long])
bind(X, "XMapWindow", [D, W])
bind(X, "XLowerWindow", [D, W])
bind(X, "XDestroyWindow", [D, W])
bind(X, "XCheckWindowEvent", [D, W, C.c_long, D])
bind(X, "XWarpPointer", [D, W, W, I, I, U, U, I, I])
bind(T, "XTestFakeButtonEvent", [D, U, I, W])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary")
    parser.add_argument("screenshot")
    parser.add_argument("mode", choices=["drag", "drag-edges", "position", "click-through", "clock", "input", "pin", "idle"], default="clock", nargs="?")
    parser.add_argument("--production", action="store_true", help="check the default 500x500 window")
    args = parser.parse_args()
    display = X.XOpenDisplay(None)
    assert display, "X11 display is unavailable"
    root = X.XDefaultRootWindow(display)

    def atom(name):
        return X.XInternAtom(display, name.encode(), 0)

    def property32(window, name, kind):
        actual, fmt, count, remaining = W(), I(), W(), W()
        data = C.POINTER(C.c_ubyte)()
        X.XGetWindowProperty(display, window, atom(name), 0, 64, 0, kind,
                             C.byref(actual), C.byref(fmt), C.byref(count),
                             C.byref(remaining), C.byref(data))
        values = list(C.cast(data, C.POINTER(W))[:count.value]) if fmt.value == 32 else []
        X.XFree(data)
        return values

    def children(window):
        r, parent, data, count = W(), W(), C.POINTER(W)(), U()
        X.XQueryTree(display, window, C.byref(r), C.byref(parent), C.byref(data), C.byref(count))
        values = list(data[:count.value])
        X.XFree(data)
        return values

    def find(window, pid, depth=0):
        if property32(window, "_NET_WM_PID", 6) == [pid]:
            return window
        if depth < 3:
            for child in children(window):
                found = find(child, pid, depth + 1)
                if found:
                    return found
        return None

    def position(window):
        x, y, child = I(), I(), W()
        X.XTranslateCoordinates(display, window, root, 0, 0, C.byref(x), C.byref(y), C.byref(child))
        return x.value, y.value

    def size(window):
        r, x, y, width, height, border, depth = W(), I(), I(), U(), U(), U(), U()
        X.XGetGeometry(display, window, C.byref(r), C.byref(x), C.byref(y), C.byref(width), C.byref(height), C.byref(border), C.byref(depth))
        return width.value, height.value

    def pointer():
        r, child, x, y, wx, wy, mask = W(), W(), I(), I(), I(), I(), U()
        X.XQueryPointer(display, root, C.byref(r), C.byref(child), C.byref(x), C.byref(y), C.byref(wx), C.byref(wy), C.byref(mask))
        return x.value, y.value

    def warp(x, y):
        X.XWarpPointer(display, 0, root, 0, 0, 0, 0, x, y)
        X.XSync(display, 0)
        time.sleep(0.12)
        assert pointer() == (x, y), ("pointer warp", pointer(), (x, y))

    def button(pressed):
        T.XTestFakeButtonEvent(display, 1, int(pressed), 0)
        X.XSync(display, 0)
        time.sleep(0.12)

    process = None
    underlying = None
    saved_pointer = pointer()
    try:
        command = [args.binary, "--screenshot", args.screenshot, "--screenshot-delay",
                   "20" if args.mode in ("drag-edges", "click-through") else "7"]
        if not args.production:
            command.append("--dev")
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        deadline = time.monotonic() + 10
        window = None
        while not window and time.monotonic() < deadline:
            assert process.poll() is None, process.stderr.read().decode()
            window = find(root, process.pid)
            time.sleep(0.05)
        assert window, "native window did not appear"
        time.sleep(1)
        names = []
        for value in property32(window, "_NET_WM_STATE", 4):
            name = X.XGetAtomName(display, value)
            names.append(C.string_at(name).decode())
            X.XFree(name)
        assert {"_NET_WM_STATE_ABOVE", "_NET_WM_STATE_SKIP_TASKBAR", "_NET_WM_STATE_SKIP_PAGER"} <= set(names), names
        original, original_size = position(window), size(window)
        assert original_size == ((500, 500) if args.production else (800, 600)), original_size
        print(json.dumps(dict(position=original, size=original_size, states=names)))
        X.XRaiseWindow(display, window)
        X.XSetInputFocus(display, window, 2, 0)
        X.XSync(display, 0)
        time.sleep(0.2)
        if args.mode == "click-through":
            # A real window below the pet must receive blank-area clicks.
            underlying = X.XCreateSimpleWindow(display, root, *original, *original_size, 0, 0, 0)
            X.XChangeWindowAttributes(display, underlying, 1 << 9,
                                      C.byref(WindowAttributes(override_redirect=1)))
            X.XSelectInput(display, underlying, 1 << 2)
            X.XMapWindow(display, underlying)
            X.XLowerWindow(display, underlying)
            X.XSync(display, 0)
            time.sleep(0.3)
            event = (C.c_long * 24)()
            for offset in [(5, 5), (original_size[0] - 5, 5), (80, 80)]:
                warp(original[0] + offset[0], original[1] + offset[1])
                button(True)
                button(False)
                assert X.XCheckWindowEvent(display, underlying, 1 << 2, C.byref(event)), ("blank area blocked click", offset)
            warp(original[0] + original_size[0] // 2 - 60, original[1] + original_size[1] // 2 - 20)
            button(True)
            button(False)
            assert not X.XCheckWindowEvent(display, underlying, 1 << 2, C.byref(event)), "pet click passed through"
            sidebar_x = max(10, original_size[0] // 2 - 315) + 25
            warp(original[0] + sidebar_x, original[1] + original_size[1] // 2 - 125)
            button(True)
            button(False)
            assert not X.XCheckWindowEvent(display, underlying, 1 << 2, C.byref(event)), "sidebar click passed through"
            print("Blank areas deliver clicks to underlying window; pet and sidebar retain clicks: passed")
        elif args.mode == "drag-edges":
            width, height = size(root)
            # Grab visible artwork; the bundled GIF's canvas center is transparent.
            start = (original[0] + original_size[0] // 2 - 60,
                     original[1] + original_size[1] // 2 - 20)
            warp(*start)
            button(True)
            # Pet center follows the pointer while transparent margins cross screen edges.
            for target in [(width - 2, height - 2), (2, height - 2), (2, 2), (width - 2, 2), start]:
                current = pointer()
                steps = max(abs(target[0] - current[0]), abs(target[1] - current[1])) // 120 + 1
                for step in range(1, steps + 1):
                    warp(round(current[0] + (target[0] - current[0]) * step / steps),
                         round(current[1] + (target[1] - current[1]) * step / steps))
                expected = (original[0] + target[0] - start[0],
                            original[1] + target[1] - start[1])
                deadline = time.monotonic() + 0.6
                while position(window) != expected and time.monotonic() < deadline:
                    time.sleep(0.02)
                assert position(window) == expected, ("screen edge clamped", position(window), expected)
                assert size(window) == original_size, "edge drag changed window dimensions"
                if target != start:
                    button(False)
                    time.sleep(0.3)
                    assert position(window) == expected, "window snapped back after releasing at screen edge"
                    button(True)
            button(False)
            time.sleep(0.3)
            assert position(window) == original, "window moved back after releasing drag"
            print("Pet drag anchor reaches all screen corners; transparent margins can leave screen: passed")
        elif args.mode == "drag":
            start = (original[0] + original_size[0] // 2 - 60,
                     original[1] + original_size[1] // 2 - 20)
            warp(*start)
            button(True)
            warp(start[0] + 3, start[1] + 2)
            assert position(window) == original, "movement below 5px moved the window"
            # Includes reversal and points outside the original circular hit region.
            for dx, dy in [(10, 5), (80, 40), (350, 50), (40, -20), (80, 40)]:
                warp(start[0] + dx, start[1] + dy)
                expected = (original[0] + dx, original[1] + dy)
                deadline = time.monotonic() + 0.6
                while position(window) != expected and time.monotonic() < deadline:
                    time.sleep(0.02)
                assert position(window) == expected, (position(window), expected)
                assert size(window) == original_size, "drag changed window dimensions"
            button(False)
            print("Drag threshold, distance, reversal, capture and size: passed")
        elif args.mode == "position":
            width, height = size(root)
            start = (original[0] + original_size[0] // 2 - 60,
                     original[1] + original_size[1] // 2 - 20)
            # Leave transparent margins offscreen while the actual artwork stays reachable.
            expected = (width - original_size[0] // 2 + 46,
                        height - original_size[1] // 2 - 18)
            target = (start[0] + expected[0] - original[0],
                      start[1] + expected[1] - original[1])
            warp(*start)
            button(True)
            for step in range(1, 6):
                warp(round(start[0] + (target[0] - start[0]) * step / 5),
                     round(start[1] + (target[1] - start[1]) * step / 5))
            button(False)
            assert position(window) == expected, (position(window), expected)
        elif args.mode in ("clock", "pin"):
            sidebar_x = original_size[0] / 2 - 315
            if sidebar_x < 0:
                sidebar_x = 10
            warp(int(original[0] + sidebar_x + 25),
                 original[1] + original_size[1] // 2 - 125 + (250 if args.mode == "clock" else 0))
            button(True)
            button(False)
        elif args.mode == "input":
            warp(original[0] + original_size[0] // 2, original[1] + original_size[1] - 40)
        if args.mode != "input":
            warp(0, 0)
        assert process.wait(timeout=25 if args.mode in ("drag-edges", "click-through") else 15) == 0, process.stderr.read().decode()
        if args.mode == "position":
            process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            deadline = time.monotonic() + 10
            window = None
            while not window and time.monotonic() < deadline:
                assert process.poll() is None, process.stderr.read().decode()
                window = find(root, process.pid)
                time.sleep(0.05)
            assert window, "reopened window did not appear"
            time.sleep(1)
            assert position(window) == expected, ("position was not restored", position(window), expected)
            assert size(window) == original_size, "restoring position changed the window size"
            assert process.wait(timeout=15) == 0, process.stderr.read().decode()
            print("Close and reopen preserves exact position, offscreen transparent margins and window size: passed")
    finally:
        T.XTestFakeButtonEvent(display, 1, 0, 0)
        X.XSync(display, 0)
        if process is not None:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            error = process.stderr.read().decode()
            if error:
                print(error)
        if underlying is not None:
            X.XDestroyWindow(display, underlying)
        X.XWarpPointer(display, 0, root, 0, 0, 0, 0, *saved_pointer)
        X.XSync(display, 0)
        X.XCloseDisplay(display)


if __name__ == "__main__":
    main()
