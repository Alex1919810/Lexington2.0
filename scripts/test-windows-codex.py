import base64
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import windows_codex as codex


class WindowsCodexTests(unittest.TestCase):
    def archive(self, path, names):
        with tarfile.open(path, 'w:gz') as archive:
            for name in names:
                member = tarfile.TarInfo(name)
                member.size = 7
                archive.addfile(member, io.BytesIO(b'fixture'))

    def test_verified_cached_package_retains_complete_layout_without_network(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cache = root / 'cache'
            cache.mkdir()
            archive = cache / f'codex-{codex.VERSION}-win32-x64.tgz'
            names = [f'package/{name}' for name in codex.REQUIRED if name != 'LICENSE']
            names.append(f'package/vendor/{codex.TARGET}/codex-resources/voice/licenses/LGPL-2.1.txt')
            self.archive(archive, names)
            license_path = cache / f'LICENSE-{codex.VERSION}'
            license_path.write_bytes(b'fixture license')
            integrity = base64.b64encode(codex.checksum(archive)).decode()
            with patch.object(codex, 'INTEGRITY', integrity), patch.object(
                    codex, 'LICENSE_SHA256', codex.checksum(license_path, 'sha256').hex()), patch.object(
                    codex, 'fetch', side_effect=AssertionError('Unexpected network')):
                destination = root / 'stage'
                codex.stage(destination, cache)
                for name in codex.REQUIRED:
                    self.assertTrue((destination / name).is_file(), name)
                self.assertTrue((destination / names[-1][len('package/'):]).is_file())
                with self.assertRaisesRegex(RuntimeError, 'must not exist'):
                    codex.stage(destination, cache)
                archive.write_bytes(b'corrupt cache')
                with self.assertRaisesRegex(RuntimeError, 'SHA-512 mismatch'):
                    codex.stage(root / 'rejected', cache)
                self.assertFalse((root / 'rejected').exists())

    def test_archive_rejects_traversal_links_and_case_collisions_before_extracting(self):
        for names in (['package/../escaped'], ['package/C:/escaped'], ['package/a\\escaped'],
                      ['/package/escaped'], ['package/NUL'], ['package/file.'],
                      ['package/file', 'package/FILE']):
            with self.subTest(names=names), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                archive = root / 'fixture.tgz'
                self.archive(archive, names)
                with self.assertRaises(RuntimeError):
                    codex.unpack(archive, root / 'output')
                self.assertFalse((root / 'output').exists())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / 'fixture.tgz'
            with tarfile.open(archive, 'w:gz') as output:
                member = tarfile.TarInfo('package/link')
                member.type = tarfile.SYMTYPE
                member.linkname = '../../escaped'
                output.addfile(member)
            with self.assertRaises(RuntimeError):
                codex.unpack(archive, root / 'output')

    def test_download_checksum_failure_does_not_publish_cache_or_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def corrupt_download(url, target):
                target.write_bytes(b'corrupt download')
            with patch.object(codex, 'fetch', side_effect=corrupt_download):
                with self.assertRaisesRegex(RuntimeError, 'SHA-512 mismatch'):
                    codex.stage(root / 'stage', root / 'cache')
            self.assertFalse((root / 'stage').exists())
            self.assertEqual(list((root / 'cache').iterdir()), [])


if __name__ == '__main__':
    unittest.main()
