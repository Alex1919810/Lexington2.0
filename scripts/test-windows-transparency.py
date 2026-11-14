"""Check desktop composition without injecting pointer or keyboard input."""

import argparse
import ctypes
from ctypes import wintypes
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time


spec = importlib.util.spec_from_file_location('native', Path(__file__).with_name('windows-smoke.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
native.user32.GetDC.argtypes = [wintypes.HWND]
native.user32.GetDC.restype = wintypes.HDC
native.user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
native.gdi32.GetPixel.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int]
native.gdi32.GetPixel.restype = wintypes.DWORD


class BlurBehind(ctypes.Structure):
    _fields_ = [('flags', wintypes.DWORD), ('enabled', wintypes.BOOL),
                ('region', wintypes.HRGN), ('transition', wintypes.BOOL)]


dwm = ctypes.WinDLL('dwmapi')
dwm.DwmEnableBlurBehindWindow.argtypes = [wintypes.HWND, ctypes.POINTER(BlurBehind)]


def pixel(point):
    context = native.user32.GetDC(None)
    try:
        return native.gdi32.GetPixel(context, round(point[0]), round(point[1]))
    finally:
        native.user32.ReleaseDC(None, context)


def run(binary, resource=None, reset_composition=False, artwork=None):
    assert not native.find_window(None, 'Lexington 桌面宠物'), 'Close existing Lexington before isolated verification'
    focus = native.user32.GetForegroundWindow()
    processes = []
    handles = []
    with tempfile.TemporaryDirectory(prefix='lexington-composition-') as directory:
        root = Path(directory)
        (root / 'resource').mkdir()
        if resource:
            shutil.copytree(resource, root / 'resource', dirs_exist_ok=True)
        if artwork:
            if artwork.name == 'actor.json':
                destination = root / 'resource/illustration/fixture/live2d/Fixture_Normal'
                shutil.copytree(artwork.parent, destination)
            else:
                destination = root / 'resource/illustration/fixture' / artwork.suffix[1:].lower()
                destination.mkdir(parents=True)
                shutil.copyfile(artwork, destination / ('Fixture_Normal' + artwork.suffix.lower()))
        (root / 'config').mkdir()
        (root / 'config/config.json').write_text(json.dumps({'clockIn': {'autoStart': False}}), encoding='utf-8')
        env = dict(os.environ, LEXINGTON_HOME=str(root), CODEX_HOME=str(root / 'codex'),
                   LEXINGTON_CODEX_EXECUTABLE=str(root / 'unavailable.exe'),
                   HOME=str(root), USERPROFILE=str(root), APPDATA=str(root / 'roaming'),
                   LOCALAPPDATA=str(root / 'local'), XDG_DATA_HOME=str(root / 'data'))
        for key in ('OPENAI_API_KEY', 'LEXINGTON_MODEL_API_KEY'):
            env.pop(key, None)
        try:
            background = subprocess.Popen([sys.executable, str(Path(__file__).with_name('windows-smoke.py')),
                                           '--underlay', str(root / 'counter')])
            processes.append(background)
            base = native.wait_for(lambda: native.find_window(background.pid, 'Lexington smoke underlay'))
            handles.append(base)
            process = subprocess.Popen([str(binary)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            processes.append(process)
            handle = native.wait_for(lambda: native.find_window(process.pid, 'Lexington 桌面宠物'))
            handles.append(handle)
            time.sleep(2)
            pointer = wintypes.POINT()
            native.user32.GetCursorPos(ctypes.byref(pointer))
            width = native.rectangle(handle)[2]
            scale = width / 500
            for index, interactive in enumerate((False, True, False, True)):
                if index and reset_composition:
                    assert dwm.DwmEnableBlurBehindWindow(handle, ctypes.byref(BlurBehind(1, False, None, False))) == 0
                left = round(pointer.x - (35 if interactive else 510) * scale)
                top = round(pointer.y - 115 * scale)
                native.user32.SetWindowPos(handle, -1, left, top, 0, 0, 0x15)
                time.sleep(0.6)
                style = native.user32.GetWindowLongW(handle, -20)
                assert bool(style & 0x20) != interactive, ('Unexpected hit-test state', hex(style), interactive)
                assert native.user32.WindowFromPoint(pointer) == (handle if interactive else base), 'Native cross-process hit test changed'
                for color in (0xFFFFFF, 0x302010):
                    native.user32.PostMessageW(base, 0x8001, color, 0)
                    time.sleep(0.3)
                    points = [(left + offset_x * scale, top + offset_y * scale)
                              for offset_x, offset_y in ((8, 8), (490, 8), (8, 490), (490, 490), (80, 80), (420, 80))]
                    points = [point for point in points if 0 <= point[0] < native.user32.GetSystemMetrics(0)
                              and 0 <= point[1] < native.user32.GetSystemMetrics(1)]
                    assert points, 'No on-screen transparent sample'
                    for point in points:
                        actual = pixel(point)
                        assert actual == color, ('Opaque desktop background', interactive, hex(style), point, hex(actual), hex(color))
        finally:
            for handle in reversed(handles):
                native.user32.PostMessageW(handle, 0x0010, 0, 0)
            for process in reversed(processes):
                try:
                    process.wait(timeout=8)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            if focus:
                native.user32.SetForegroundWindow(focus)
    print('Windows desktop alpha passed: white/dark underlays, interactive/passthrough transitions; no input injected.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('binary', type=Path)
    parser.add_argument('--resource', type=Path)
    parser.add_argument('--artwork', type=Path)
    parser.add_argument('--reset-composition', action='store_true')
    arguments = parser.parse_args()
    run(arguments.binary.resolve(), arguments.resource, arguments.reset_composition, arguments.artwork)
