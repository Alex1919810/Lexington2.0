import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time


user32 = ctypes.WinDLL('user32', use_last_error=True)
kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)
gdi32 = ctypes.WinDLL('gdi32', use_last_error=True)
callback_type = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
enum_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
user32.DefWindowProcW.restype = ctypes.c_ssize_t
user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.CreateWindowExW.restype = wintypes.HWND
user32.GetForegroundWindow.restype = wintypes.HWND
user32.WindowFromPoint.argtypes = [wintypes.POINT]
user32.WindowFromPoint.restype = wintypes.HWND
user32.GetWindowLongW.argtypes = [wintypes.HWND, ctypes.c_int]
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.EnumWindows.argtypes = [enum_type, wintypes.LPARAM]
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.SetWindowPos.argtypes = [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT]
kernel32.GetModuleHandleW.restype = wintypes.HMODULE
gdi32.GetStockObject.restype = wintypes.HGDIOBJ
try:
    user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
except AttributeError:
    user32.SetProcessDPIAware()


def wait_for(predicate, timeout=12):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError('Timed out waiting for native Windows state')


def remove_executable(path):
    try:
        path.unlink(missing_ok=True)
    except OSError as error:
        if error.winerror not in (32, 33):
            raise
        return False
    return True


def find_window(pid, title):
    found = []

    @enum_type
    def visit(handle, unused):
        owner = wintypes.DWORD()
        user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
        text = ctypes.create_unicode_buffer(256)
        user32.GetWindowTextW(handle, text, len(text))
        if (pid is None or owner.value == pid) and text.value == title and user32.IsWindowVisible(handle):
            found.append(handle)
        return True

    user32.EnumWindows(visit, 0)
    return found[0] if found else None


def rectangle(handle):
    value = wintypes.RECT()
    assert user32.GetWindowRect(handle, ctypes.byref(value))
    return value.left, value.top, value.right - value.left, value.bottom - value.top


def move_pointer(point):
    assert user32.SetCursorPos(round(point[0]) + 1, round(point[1]))
    time.sleep(0.15)
    assert user32.SetCursorPos(round(point[0]), round(point[1]))
    time.sleep(0.15)


def click(point, right=False):
    move_pointer(point)
    user32.mouse_event(0x0008 if right else 0x0002, 0, 0, 0, 0)
    time.sleep(0.08)
    user32.mouse_event(0x0010 if right else 0x0004, 0, 0, 0, 0)
    time.sleep(0.25)


def drag(handle, start, end):
    move_pointer(start)
    wait_for(lambda: user32.WindowFromPoint(wintypes.POINT(round(start[0]), round(start[1]))) == handle)
    user32.mouse_event(0x0002, 0, 0, 0, 0)
    try:
        time.sleep(0.15)
        for step in range(1, 16):
            point = [start[index] + (end[index] - start[index]) * step / 15 for index in (0, 1)]
            assert user32.SetCursorPos(round(point[0]), round(point[1]))
            time.sleep(0.035)
    finally:
        user32.mouse_event(0x0004, 0, 0, 0, 0)
    time.sleep(0.4)


def underlay(path):
    class WindowClass(ctypes.Structure):
        _fields_ = [('style', wintypes.UINT), ('procedure', callback_type), ('class_extra', ctypes.c_int),
                    ('window_extra', ctypes.c_int), ('instance', wintypes.HINSTANCE), ('icon', wintypes.HICON),
                    ('cursor', wintypes.HANDLE), ('background', wintypes.HBRUSH),
                    ('menu', wintypes.LPCWSTR), ('name', wintypes.LPCWSTR)]

    count = 0

    @callback_type
    def procedure(handle, message, first, second):
        nonlocal count
        if message == 0x0202:
            count += 1
            path.write_text(str(count), encoding='ascii')
        if message == 0x0002:
            user32.PostQuitMessage(0)
            return 0
        return user32.DefWindowProcW(handle, message, first, second)

    instance = kernel32.GetModuleHandleW(None)
    window_class = WindowClass(0, procedure, 0, 0, instance, None, None,
                               gdi32.GetStockObject(1), None, 'LexingtonSmokeUnderlay')
    assert user32.RegisterClassW(ctypes.byref(window_class))
    user32.CreateWindowExW.argtypes = [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
                                     ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                     wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID]
    handle = user32.CreateWindowExW(0, window_class.name, 'Lexington smoke underlay', 0x90000000,
                                   0, 0, user32.GetSystemMetrics(0), user32.GetSystemMetrics(1),
                                   None, None, instance, None)
    assert handle
    user32.SetForegroundWindow(handle)
    path.write_text('0', encoding='ascii')
    message = wintypes.MSG()
    while user32.GetMessageW(ctypes.byref(message), None, 0, 0) > 0:
        user32.TranslateMessage(ctypes.byref(message))
        user32.DispatchMessageW(ctypes.byref(message))


def run(binary, artwork=None, pet_point=(250, 250)):
    assert not find_window(None, 'Lexington 桌面宠物'), 'Close existing Lexington before isolated native verification'
    original_pointer = wintypes.POINT()
    user32.GetCursorPos(ctypes.byref(original_pointer))
    original_focus = user32.GetForegroundWindow()
    owned = []
    handles = []
    with tempfile.TemporaryDirectory(prefix='lexington-native-windows-') as directory:
        root = Path(directory)
        (root / 'resource').mkdir()
        if artwork:
            images = root / 'resource/illustration/fixture' / artwork.suffix[1:].lower()
            images.mkdir(parents=True)
            shutil.copy2(artwork, images / ('Fixture_Normal' + artwork.suffix.lower()))
        (root / 'config').mkdir()
        (root / 'config/config.json').write_text(json.dumps({
            'codex': {'executable': str(root / 'deliberately-unavailable.exe')},
            'clockIn': {'autoStart': False},
        }), encoding='utf-8')
        env = dict(os.environ, LEXINGTON_HOME=str(root), CODEX_HOME=str(root / 'codex'), LEXINGTON_CODEX_EXECUTABLE=str(root / 'deliberately-unavailable.exe'),
                   APPDATA=str(root / 'roaming'), LOCALAPPDATA=str(root / 'local'),
                   HOME=str(root), USERPROFILE=str(root))
        for credential in ("OPENAI_API_KEY", "LEXINGTON_MODEL_API_KEY"):
            env.pop(credential, None)
        counter = root / 'clicks.txt'
        alias = root / 'lexington-native-host.exe'

        def launch():
            process = subprocess.Popen([str(binary)], env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            owned.append(process)
            handle = wait_for(lambda: find_window(process.pid, 'Lexington 桌面宠物'))
            handles.append(handle)
            time.sleep(1.2)
            return process, handle

        try:
            background = subprocess.Popen([sys.executable, __file__, '--underlay', str(counter)])
            owned.append(background)
            base = wait_for(lambda: find_window(background.pid, 'Lexington smoke underlay'))
            handles.append(base)
            wait_for(counter.exists)
            click((20, 20), right=True)
            wait_for(lambda: user32.GetForegroundWindow() == base)
            process, handle = launch()
            assert user32.GetForegroundWindow() == base, 'Pet stole foreground focus at startup'
            left, top, width, height = rectangle(handle)
            scale = width / 500
            offset = (pet_point[0] * scale, pet_point[1] * scale)
            assert abs(height - width) <= 2, (width, height)
            assert user32.GetWindowLongW(handle, -20) & 0x8, 'Pet is not topmost'
            assert not user32.GetWindowLongW(handle, -20) & 0x40000, 'Pet has APPWINDOW/taskbar style'
            user32.SetWindowPos(handle, None, round(100 * scale), round(100 * scale), 0, 0, 0x15)
            time.sleep(0.4)
            left, top, width, height = rectangle(handle)
            click((left + 8 * scale, top + 8 * scale))
            wait_for(lambda: counter.read_text() == '1')
            click((left + offset[0], top + offset[1]))
            assert counter.read_text() == '1', 'Visible pet passed its click to another process'
            click((left + 35 * scale, top + 115 * scale))
            assert counter.read_text() == '1', ('Sidebar passed its click through', rectangle(handle), hex(user32.GetWindowLongW(handle, -20)), scale)
            start = (left + offset[0], top + offset[1])
            drag(handle, start, (start[0] + 70 * scale, start[1] + 45 * scale))
            moved = rectangle(handle)
            assert abs(moved[0] - left - 70 * scale) <= 3 and abs(moved[1] - top - 45 * scale) <= 3, (moved, left, top)
            assert moved[2:] == (width, height)
            center = (moved[0] + offset[0], moved[1] + offset[1])
            click(center, right=True)
            click((center[0] + 23 * scale, center[1] + 12 * scale))
            settings = wait_for(lambda: find_window(process.pid, 'Lexington 设置'))
            handles.append(settings)
            time.sleep(1)
            before = rectangle(settings)
            title = (before[0] + 150 * scale, before[1] + 17 * scale)
            drag(settings, title, (title[0] + 60 * scale, title[1] + 35 * scale))
            after = rectangle(settings)
            assert abs(after[0] - before[0] - 60 * scale) <= 3 and abs(after[1] - before[1] - 35 * scale) <= 3, (before, after)
            assert rectangle(handle) == moved, 'Settings drag moved the pet'
            click((after[0] + after[2] - 26 * scale, after[1] + 26 * scale))
            wait_for(lambda: not find_window(process.pid, 'Lexington 设置'))
            before = rectangle(handle)
            for target in [(25, 25), (user32.GetSystemMetrics(0) - 25, 25),
                           (user32.GetSystemMetrics(0) - 25, user32.GetSystemMetrics(1) - 25),
                           (25, user32.GetSystemMetrics(1) - 25)]:
                current = rectangle(handle)
                drag(handle, (current[0] + offset[0], current[1] + offset[1]), target)
                edge = rectangle(handle)
                assert abs(edge[0] + offset[0] - target[0]) <= 3 and abs(edge[1] + offset[1] - target[1]) <= 3, (edge, target)
                assert edge[2:] == (width, height)
            current = rectangle(handle)
            drag(handle, (current[0] + offset[0], current[1] + offset[1]), (before[0] + offset[0], before[1] + offset[1]))
            saved = rectangle(handle)
            user32.PostMessageW(handle, 0x0010, 0, 0)
            assert process.wait(timeout=10) == 0
            process, handle = launch()
            assert rectangle(handle) == saved, ('Position was not restored', saved, rectangle(handle))
            selection = {'text': 'Windows 原生通信夹具', 'files': [], 'source': 'fixture'}
            result = subprocess.run([str(binary), '--ask-json'], input=json.dumps(selection).encode(),
                                    env=env, capture_output=True, timeout=8)
            assert result.returncode == 0, 'ask-json failed'
            shutil.copy2(binary, alias)
            payload = json.dumps(selection).encode()
            result = subprocess.run([str(alias)], input=struct.pack('<I', len(payload)) + payload,
                                    env=env, capture_output=True, timeout=8)
            assert result.returncode == 0 and len(result.stdout) >= 4, 'Release native host framing failed'
            length, = struct.unpack('<I', result.stdout[:4])
            assert len(result.stdout) == length + 4 and json.loads(result.stdout[4:])['ok'] is True
        finally:
            user32.mouse_event(0x0004 | 0x0010, 0, 0, 0, 0)
            living = {process.pid for process in owned if process.poll() is None}
            for handle in reversed(handles):
                owner = wintypes.DWORD()
                user32.GetWindowThreadProcessId(handle, ctypes.byref(owner))
                if owner.value in living:
                    user32.PostMessageW(handle, 0x0010, 0, 0)
            for process in reversed(owned):
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            user32.SetCursorPos(original_pointer.x, original_pointer.y)
            if original_focus:
                user32.SetForegroundWindow(original_focus)
            wait_for(lambda: remove_executable(alias))
    print('Windows native smoke passed: cross-process click-through, focus/topmost/taskbar, pet/settings drag, four edges, position restore, JSON/native host.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('binary', nargs='?')
    parser.add_argument('--underlay', type=Path)
    parser.add_argument('--artwork', type=Path)
    parser.add_argument('--pet-point', type=float, nargs=2, default=(250, 250))
    arguments = parser.parse_args()
    if arguments.underlay:
        underlay(arguments.underlay)
    else:
        run(Path(arguments.binary).resolve(), arguments.artwork.resolve() if arguments.artwork else None, arguments.pet_point)
