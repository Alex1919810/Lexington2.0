#!/usr/bin/env python3
"""Native settings drag regression. Use an isolated X11 display when possible.

Exercises title/body dragging, controls, focus, reopening and 1x/1.5x DPI.
Uses a temporary resource root (fallback pet) and no real Codex credentials.
"""
import argparse
import ctypes as C
import json
import os
import pathlib
import runpy
import signal
import subprocess
import tempfile
import time
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=pathlib.Path)
args = parser.parse_args()
binary = args.binary.resolve()
project = pathlib.Path(__file__).resolve().parent.parent
m = runpy.run_path(str(project / 'scripts/native-smoke.py'))
X, T, bind = (m['X'], m['T'], m['bind'])
D, W, I, U = (m['D'], m['W'], m['I'], m['U'])
temporary = tempfile.TemporaryDirectory(prefix='lexington-settings-smoke-')
base = pathlib.Path(temporary.name)
(base / 'config').mkdir()
(base / 'run').mkdir(mode=448)
(base / 'config/config.json').write_text(json.dumps({'clockIn': {'autoStart': False}, 'codex': {'executable': str(base / 'missing-codex')}}))
d = X.XOpenDisplay(None)
assert d
root = X.XDefaultRootWindow(d)

def children(w):
    rr, pa, data, n = (W(), W(), C.POINTER(W)(), U())
    X.XQueryTree(d, w, C.byref(rr), C.byref(pa), C.byref(data), C.byref(n))
    v = list(data[:n.value])
    X.XFree(data)
    return v

def prop(w, name, kind=6):
    actual, fmt, n, left = (W(), I(), W(), W())
    data = C.POINTER(C.c_ubyte)()
    atom = X.XInternAtom(d, name.encode(), 0)
    X.XGetWindowProperty(d, w, atom, 0, 1000, 0, kind, C.byref(actual), C.byref(fmt), C.byref(n), C.byref(left), C.byref(data))
    v = list(C.cast(data, C.POINTER(W))[:n.value]) if fmt.value == 32 else bytes(data[:n.value]) if fmt.value == 8 else b''
    X.XFree(data)
    return v

def find(pid):
    all = []

    def scan(w, depth):
        if prop(w, '_NET_WM_PID') == [pid]:
            all.append(w)
        if depth < 3:
            for c in children(w):
                scan(c, depth + 1)
    scan(root, 0)
    return all

def geometry(w):
    x, y, child = (I(), I(), W())
    X.XTranslateCoordinates(d, w, root, 0, 0, C.byref(x), C.byref(y), C.byref(child))
    r, xx, yy, ww, hh, b, dep = (W(), I(), I(), U(), U(), U(), U())
    X.XGetGeometry(d, w, C.byref(r), C.byref(xx), C.byref(yy), C.byref(ww), C.byref(hh), C.byref(b), C.byref(dep))
    return (x.value, y.value, ww.value, hh.value)

def click(x, y, button=1):
    X.XWarpPointer(d, 0, root, 0, 0, 0, 0, x, y)
    X.XSync(d, 0)
    time.sleep(0.2)
    for pressed in [1, 0]:
        T.XTestFakeButtonEvent(d, button, pressed, 0)
        X.XSync(d, 0)
        time.sleep(0.15)

def waitfor(fn, timeout=8):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        found = fn()
        if found:
            return found
        time.sleep(0.1)
    raise AssertionError('timed out')

def launch(env, label):
    return subprocess.Popen([str(binary)], env=env, stdout=subprocess.DEVNULL, stderr=(base / (label + '.log')).open('w'), start_new_session=True)

def stop(p):
    if p.poll() is None:
        os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=5)

def warp(x, y):
    X.XWarpPointer(d, 0, root, 0, 0, 0, 0, int(x), int(y))
    X.XSync(d, 0)
    time.sleep(0.15)

def drag(w, dx, dy, scale, local=(100, 24)):
    g = geometry(w)
    x, y = (g[0] + int(local[0] * scale), g[1] + int(local[1] * scale))
    warp(x, y)
    T.XTestFakeButtonEvent(d, 1, 1, 0)
    X.XSync(d, 0)
    time.sleep(0.15)
    try:
        for fraction in [0.02, 0.1, 0.3, 0.6, 1]:
            warp(x + dx * fraction, y + dy * fraction)
    finally:
        T.XTestFakeButtonEvent(d, 1, 0, 0)
        X.XSync(d, 0)
    time.sleep(0.5)
    end = geometry(w)
    assert abs(end[0] - g[0] - dx) < 12 and abs(end[1] - g[1] - dy) < 12, (g, end, dx, dy)
    return end
r, c, px, py, wx, wy, mask = (W(), W(), I(), I(), I(), I(), U())
X.XQueryPointer(d, root, C.byref(r), C.byref(c), C.byref(px), C.byref(py), C.byref(wx), C.byref(wy), C.byref(mask))
saved_pointer = (px.value, py.value)
try:
    for scale in [1, 1.5]:
        label = str(scale)
        data = base / label
        data.mkdir(parents=True, exist_ok=True)
        cfg = base / 'config/config.json'
        v = json.loads(cfg.read_text())
        v['codex']['noApproval'] = False
        cfg.write_text(json.dumps(v))
        env = {**os.environ, 'LEXINGTON_HOME': str(base), 'XDG_DATA_HOME': str(data), 'XDG_RUNTIME_DIR': str(base / 'run'), 'WINIT_X11_SCALE_FACTOR': str(scale)}
        p = launch(env, label)
        try:
            pet = waitfor(lambda: next((w for w in find(p.pid) if prop(w, '_NET_WM_NAME', 0) == 'Lexington 桌面宠物'.encode()), None))
            time.sleep(1)
            X.XSetInputFocus(d, pet, 2, 0)
            X.XSync(d, 0)
            pg = geometry(pet)
            cx, cy = (pg[0] + int(250 * scale), pg[1] + int(250 * scale))
            click(cx, cy, 3)
            time.sleep(0.3)
            click(cx + int(20 * scale), cy + int(12 * scale))
            settings = waitfor(lambda: next((w for w in find(p.pid) if prop(w, '_NET_WM_NAME', 0) == 'Lexington 设置'.encode()), None))
            time.sleep(1.3)
            g = geometry(settings)
            print(label, 'initial settings', g, flush=True)
            g = drag(settings, -400, 140, scale)
            assert g[0] < pg[0] - 150, 'settings remains trapped in pet window'
            assert geometry(pet) == pg, 'settings drag moved pet'
            g = drag(settings, 160, -70, scale)
            for local in [(50, 280), (270, 270)]:
                g = drag(settings, 80, 20, scale, local)
                g = drag(settings, -80, -20, scale, local)
            g = geometry(settings)
            warp(g[0] + 50 * scale, g[1] + 118 * scale)
            T.XTestFakeButtonEvent(d, 1, 1, 0)
            X.XSync(d, 0)
            time.sleep(0.15)
            try:
                warp(g[0] + 130 * scale, g[1] + 150 * scale)
            finally:
                T.XTestFakeButtonEvent(d, 1, 0, 0)
                X.XSync(d, 0)
            time.sleep(0.3)
            assert geometry(settings) == g, 'control press moved window'
            X.XSetInputFocus(d, root, 2, 0)
            X.XSync(d, 0)
            time.sleep(0.3)
            g = drag(settings, -100, 80, scale)
            click(g[0] + int(438 * scale), g[1] + int(26 * scale))
            waitfor(lambda: all((prop(w, '_NET_WM_NAME', 0) != 'Lexington 设置'.encode() for w in find(p.pid))))
            click(cx, cy, 3)
            time.sleep(0.3)
            click(cx + int(20 * scale), cy + int(12 * scale))
            settings = waitfor(lambda: next((w for w in find(p.pid) if prop(w, '_NET_WM_NAME', 0) == 'Lexington 设置'.encode()), None))
            time.sleep(1.1)
            g = drag(settings, -200, 100, scale)
            assert geometry(pet) == pg, 'body drag moved pet'
            g = geometry(settings)
            click(g[0] + int(155 * scale), g[1] + int(136 * scale))
            assert json.loads(cfg.read_text())['codex']['noApproval'] is True, 'checkbox no longer works'
            print('Title/body drag, controls, unfocus/reopen, exact delta and DPI passed', scale, flush=True)
        finally:
            stop(p)
finally:
    T.XTestFakeButtonEvent(d, 1, 0, 0)
    X.XWarpPointer(d, 0, root, 0, 0, 0, 0, *saved_pointer)
    X.XSync(d, 0)
    X.XCloseDisplay(d)
    temporary.cleanup()
