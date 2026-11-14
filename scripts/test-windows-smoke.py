import contextlib
import importlib.util
import json
from pathlib import Path
import struct
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch


if sys.platform == 'win32':
    spec = importlib.util.spec_from_file_location('native', Path(__file__).with_name('windows-smoke.py'))
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)


@unittest.skipUnless(sys.platform == 'win32', 'Windows smoke runner')
class WindowsSmokeTests(unittest.TestCase):
    def exercise(self, cases=None, fail_drag=False, fail_restore=False):
        geometry = {200: (100, 100, 500, 500), 300: (400, 200, 720, 560)}
        state = {'settings': False, 'counter': None}
        processes = []

        def spawn(command, **kwargs):
            process = Mock(pid=len(processes) + 1)
            process.poll.return_value = 0
            process.wait.return_value = 0
            processes.append(process)
            if '--underlay' in command:
                state['counter'] = Path(command[-1])
                state['counter'].write_text('0')
            elif fail_restore and len(processes) == 2:
                geometry[200] = (100, 100, 500, 500)
            return process

        def find_window(pid, title):
            if pid is None:
                return None
            if title == 'Lexington smoke underlay':
                return 100
            if title == 'Lexington 设置':
                return 300 if state['settings'] else None
            return 200

        def move_window(handle, unused, left, top, *args):
            geometry[handle] = (left, top, *geometry[handle][2:])

        def drag(handle, start, end):
            if fail_drag:
                raise AssertionError('fixture drag failure')
            before = geometry[handle]
            geometry[handle] = (round(before[0] + end[0] - start[0]),
                                round(before[1] + end[1] - start[1]), *before[2:])

        def click(point, right=False):
            if right:
                state['settings'] = True
            elif state['settings']:
                left, top, width, height = geometry[300]
                if point == (left + width - 26, top + 26):
                    state['settings'] = False
            elif point == (108, 108) and state['counter']:
                state['counter'].write_text('1')

        def wait_for(predicate):
            result = predicate()
            self.assertTrue(result, 'Mock native state was not reached')
            return result

        with tempfile.TemporaryDirectory(prefix='lexington-smoke-unit-') as directory, contextlib.ExitStack() as stack:
            binary = Path(directory) / 'fixture.exe'
            binary.write_bytes(b'fixture, never executed')
            api = stack.enter_context(patch.object(native, 'user32'))
            api.GetForegroundWindow.return_value = 100
            api.GetWindowLongW.return_value = 8
            api.GetSystemMetrics.side_effect = lambda index: (1920, 1080)[index]
            api.SetWindowPos.side_effect = move_window
            stack.enter_context(patch.object(native, 'find_window', side_effect=find_window))
            stack.enter_context(patch.object(native, 'rectangle', side_effect=lambda handle: geometry[handle]))
            stack.enter_context(patch.object(native, 'wait_for', side_effect=wait_for))
            stack.enter_context(patch.object(native.time, 'sleep'))
            stack.enter_context(patch('builtins.print'))
            launches = stack.enter_context(patch.object(native.subprocess, 'Popen', side_effect=spawn))
            payload = json.dumps({'ok': True}).encode()
            bridge = stack.enter_context(patch.object(native.subprocess, 'run', return_value=Mock(
                returncode=0, stdout=struct.pack('<I', len(payload)) + payload)))
            drags = stack.enter_context(patch.object(native, 'drag', side_effect=drag))
            clicks = stack.enter_context(patch.object(native, 'click', side_effect=click))
            if fail_drag:
                with self.assertRaisesRegex(AssertionError, 'fixture drag failure'):
                    native.run(binary, cases=cases, allow_input=True)
            elif fail_restore:
                with self.assertRaisesRegex(AssertionError, 'Position was not restored'):
                    native.run(binary, cases=cases, allow_input=True)
            else:
                native.run(binary, cases=cases, allow_input=True)
        return api, launches, bridge, drags, clicks, processes

    def test_default_selects_only_window(self):
        self.assertEqual(native.resolve_cases(), ('window',))

    def test_interactive_cases_require_permission(self):
        for case in (*native.INPUT_CASES, 'all'):
            with self.subTest(case=case), patch.object(native, 'find_window') as find_window:
                with self.assertRaisesRegex(ValueError, '--allow-input'):
                    native.run(Path('not-executed.exe'), cases=[case])
                find_window.assert_not_called()

    def test_unknown_case_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'Unknown smoke cases'):
            native.resolve_cases(['not-a-case'])

    def test_selection_is_deduplicated_and_ordered(self):
        self.assertEqual(native.resolve_cases(['position', 'drag', 'drag'], True), ('drag', 'position'))
        self.assertEqual(native.resolve_cases(['all', 'settings'], True), native.CASES)

    def test_non_input_cases_do_not_inject_even_during_cleanup(self):
        for cases in (None, ['bridge'], ['window', 'bridge']):
            with self.subTest(cases=cases):
                api, launches, bridge, drags, clicks, processes = self.exercise(cases)
                drags.assert_not_called()
                clicks.assert_not_called()
                api.mouse_event.assert_not_called()
                api.SetCursorPos.assert_not_called()
                api.SetWindowPos.assert_not_called()
                self.assertEqual(bridge.call_count, 2 if cases and 'bridge' in cases else 0)
                self.assertTrue(all(process.wait.called for process in processes))

    def test_each_case_runs_only_its_own_drags(self):
        for case, expected in [('click-through', 0), ('drag', 1), ('settings', 1),
                               ('drag-edges', 4), ('position', 1)]:
            with self.subTest(case=case):
                api, launches, bridge, drags, clicks, processes = self.exercise([case])
                self.assertEqual(drags.call_count, expected)
                bridge.assert_not_called()
                if case == 'drag':
                    clicks.assert_not_called()
                self.assertEqual(launches.call_count, 2 if case in ('position', 'click-through') else 1)

    def test_position_reuses_selected_pet_drag(self):
        for cases, expected in [(['drag', 'position'], 1), (['drag-edges', 'position'], 4), (['all'], 6)]:
            with self.subTest(cases=cases):
                api, launches, bridge, drags, clicks, processes = self.exercise(cases)
                self.assertEqual(drags.call_count, expected)
                self.assertEqual(launches.call_count, 3 if cases == ['all'] else 2)

    def test_failed_drag_still_releases_and_restores_input(self):
        api, launches, bridge, drags, clicks, processes = self.exercise(['drag'], fail_drag=True)
        api.mouse_event.assert_called_once_with(0x0004 | 0x0010, 0, 0, 0, 0)
        api.SetCursorPos.assert_called_once()
        self.assertTrue(all(process.wait.called for process in processes))

    def test_position_still_checks_restored_geometry(self):
        self.exercise(['position'], fail_restore=True)


if __name__ == '__main__':
    unittest.main()
