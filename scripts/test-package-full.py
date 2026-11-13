#!/usr/bin/env python3
"""Offline package fixtures: no network, real package installation, or user data."""
import base64
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / 'scripts'))
spec = importlib.util.spec_from_file_location('package_full', ROOT / 'scripts/package-full.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
deb_spec = importlib.util.spec_from_file_location('package_deb', ROOT / 'scripts/package-deb.py')
deb_builder = importlib.util.module_from_spec(deb_spec)
deb_spec.loader.exec_module(deb_builder)
from package_assets import strip_linux_binary


class OfflinePackageTests(unittest.TestCase):
    @unittest.skipUnless(shutil.which('cc') and shutil.which('strip') and shutil.which('readelf'), 'native toolchain required')
    def test_stripped_executable_runs_and_preserves_source(self):
        with tempfile.TemporaryDirectory(prefix='lexington-strip-test-') as temporary:
            root = Path(temporary)
            source = root / 'fixture.c'
            source.write_text('int main(void) { return 0; }\n')
            original = root / 'fixture'
            subprocess.run(['cc', '-g', '-o', str(original), str(source)], check=True)
            before = original.read_bytes()
            self.assertIn('.debug_info', subprocess.check_output(['readelf', '-SW', str(original)], text=True))
            stripped = root / 'packaged/lexington'
            strip_linux_binary(original, stripped)
            self.assertEqual(original.read_bytes(), before)
            sections = subprocess.check_output(['readelf', '-SW', str(stripped)], text=True)
            self.assertNotIn('.symtab', sections)
            self.assertNotIn('.debug_', sections)
            self.assertLess(stripped.stat().st_size, len(before))
            subprocess.run([str(stripped)], check=True)
            second = root / 'repacked/lexington'
            strip_linux_binary(stripped, second)
            self.assertEqual(second.read_bytes(), stripped.read_bytes())

    def test_offline_bundle_and_installer(self):
        arch = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
        native_arch, linux_arch = {'amd64': ('x86_64', 'x64'), 'arm64': ('aarch64', 'arm64')}[arch]
        with tempfile.TemporaryDirectory(prefix='lexington-offline-test-') as temporary:
            root = Path(temporary)
            source = root / 'source with spaces'
            scripts = source / 'scripts'
            scripts.mkdir(parents=True)
            for name in ('package.sh', 'install-linux.sh', 'install-dependencies.sh',
                         'install-offline-dependencies.sh', 'dependencies.py', 'launch-deb.sh', 'package_assets.py'):
                shutil.copy2(ROOT / 'scripts' / name, scripts / name)
            (scripts / 'integrations.py').write_text(
                'from pathlib import Path\nimport sys\n'
                'if sys.argv[1] == "build":\n'
                '    p = Path(sys.argv[sys.argv.index("--output") + 1])\n'
                '    p.mkdir(parents=True, exist_ok=True)\n'
                '    (p / "fixture.vsix").write_text("fixture")\n')
            (source / 'README.md').write_text('fixture\n')
            (source / 'Cargo.toml').write_text('[package]\nname = "lexington"\nversion = "2.0.0"\n')
            (source / 'integrations').mkdir()
            model = source / 'resource/models/bge-small-zh-v1.5'
            model.mkdir(parents=True)
            for name in ('model.onnx', 'config.json', 'tokenizer.json', 'tokenizer_config.json',
                         'special_tokens_map.json', 'manifest.json', 'LICENSE.txt'):
                (model / name).write_text('fixture model\n')
            drafts = source / 'resource/illustration/lexington/live2d-drafts'
            drafts.mkdir(parents=True)
            (drafts / 'preview.gif').write_text('draft fixture')
            windows = source / 'resource/runtime/windows-x86_64'
            windows.mkdir(parents=True)
            (windows / 'onnxruntime.dll').write_text('Windows DLL fixture')
            (source / 'target/release').mkdir(parents=True)
            (source / 'target/release/lexington').write_text(
                '#!/bin/sh\nprintf "%s\\n" "$@" >> "$LEXINGTON_TEST_LAUNCH_LOG"\n')
            (source / 'target/release/lexington').chmod(0o755)
            (source / 'config/characters').mkdir(parents=True)
            (source / 'config/config.json').write_text('{"apiKey":"private-fixture"}')
            (source / 'config/characters/private').write_text('private-fixture')
            # Old standard bundles can contain arbitrary extra config; full staging excludes it.
            old_config = source / 'dist/lexington/config'
            old_config.mkdir(parents=True)
            (old_config / 'old-private.json').write_text('private-fixture')
            bin_dir = root / 'bin'
            bin_dir.mkdir()
            log = root / 'apt.log'
            fake_commands = {
                'cargo': '#!/bin/sh\nexit 0\n',
                'dpkg': f'#!/bin/sh\nprintf "{arch}\\n"\n',
                'dpkg-deb': '#!/bin/sh\nif [ "$1" = -f ]; then\n'
                            f'printf "Package: fixture\\nVersion: 1\\nArchitecture: {arch}\\n"\n'
                            'else exec /usr/bin/dpkg-deb "$@"; fi\n',
                'apt-get': '#!/bin/bash\nset -eu\nprintf "%s\\n" "$@" >> "$APT_LOG"\n'
                           'for arg in "$@"; do\n'
                           '  case "$arg" in\n'
                           '    Dir::State::status=*) [[ ! -s "${arg#*=}" ]] ;;\n'
                           '    Dir::Cache::archives=*) printf fixture-deb > "${arg#*=}/fixture.deb" ;;\n'
                           '  esac\ndone\n',
                'sudo': '#!/bin/sh\nexec "$@"\n',
                'xdg-user-dir': '#!/bin/sh\nprintf "%s/Desktop\\n" "$HOME"\n',
                'gio': '#!/bin/sh\nexit 0\n',
                'update-desktop-database': '#!/bin/sh\nexit 0\n',
                'strip': '#!/bin/sh\nset -eu\n'
                         '[ "$1" = --strip-all ] && [ "$2" = -o ]\n'
                         'cp -- "$4" "$3"\n',
            }
            for name, data in fake_commands.items():
                (bin_dir / name).write_text(data)
                (bin_dir / name).chmod(0o755)
            archive_content = io.BytesIO()
            with tarfile.open(fileobj=archive_content, mode='w:gz') as archive:
                for name, data in {
                    'bin/codex': b'#!/bin/sh\necho codex-cli 0.168.0\n',
                    'bin/codex-code-mode-host': b'helper',
                    'codex-package.json': b'{}', 'codex-resources/fixture': b'resource',
                }.items():
                    member = tarfile.TarInfo(f'package/vendor/{native_arch}-unknown-linux-musl/' + name)
                    member.size, member.mode = len(data), 0o755
                    archive.addfile(member, io.BytesIO(data))
            codex_bytes = archive_content.getvalue()
            metadata = {
                'name': '@openai/codex', 'version': f'0.168.0-linux-{linux_arch}',
                'dist': {'tarball': 'https://registry.npmjs.org/fixture.tgz',
                         'integrity': 'sha512-' + base64.b64encode(hashlib.sha512(codex_bytes).digest()).decode()},
            }

            def fetch(url, target=None):
                if target:
                    Path(target).write_bytes(codex_bytes)
                elif url.endswith('/latest'):
                    return json.dumps({'name': '@openai/codex', 'version': '0.168.0'}).encode()
                else:
                    return json.dumps(metadata).encode()

            home = root / 'home'
            home.mkdir()
            env = {**os.environ, 'HOME': str(home), 'PATH': str(bin_dir) + ':/usr/bin:/bin',
                   'XDG_DATA_HOME': str(home / 'data'), 'XDG_CONFIG_HOME': str(home / 'settings'),
                   'APT_LOG': str(log), 'LEXINGTON_TEST_LAUNCH_LOG': str(root / 'launch.log')}
            output = root / 'output'
            host = subprocess.check_output(['bash', '-c', '. /etc/os-release; printf "%s\\n%s" "$ID" "$VERSION_ID"'], text=True).splitlines()
            release = {'ID': host[0], 'VERSION_ID': host[1]}
            with patch.dict(os.environ, env), \
                 patch.object(builder, '__file__', str(scripts / 'package-full.py')), \
                 patch.object(builder.platform, 'freedesktop_os_release', return_value=release), \
                 patch.object(builder.dependencies, 'fetch', side_effect=fetch):
                builder.build(output)
                filename = output / f'lexington-full-{host[0]}-{host[1]}-{arch}.tar.gz'
                assert filename.is_file()
                extracted = root / 'extracted with spaces'
                with tarfile.open(filename) as archive:
                    archive.extractall(extracted, filter='data')
                bundle = extracted / 'lexington'
                assert sorted(p.name for p in (bundle / 'config').iterdir()) == ['config.json']
                assert (bundle / 'config/config.json').read_text() == '{}\n'
                assert (source / 'config/characters/private').read_text() == 'private-fixture'
                assert (bundle / 'offline/codex.tgz').read_bytes() == codex_bytes
                assert not (bundle / 'resource/runtime/windows-x86_64').exists()
                assert not (bundle / 'resource/illustration/lexington/live2d-drafts').exists()
                assert (drafts / 'preview.gif').is_file() and (windows / 'onnxruntime.dll').is_file()
                apt_args = log.read_text()
                assert '--download-only' in apt_args and 'Dir::State::status=' in apt_args
                assert 'libgl1-mesa-dri' in apt_args and 'python3-nautilus' in apt_args
                assert not any('rust' in p or 'node' in p for p in builder.RUNTIME_PACKAGES)
                log.unlink()
                # Exercise the real installation entry, with system mutations replaced by stubs.
                subprocess.run(['bash', str(bundle / 'install.sh')], env=env, check=True)
                installed = home / 'data/lexington'
                assert (installed / 'lexington').read_bytes() == (bundle / 'lexington').read_bytes()
                assert (home / 'Desktop/Lexington.desktop').is_file()
                local_cli = home / '.local/bin/codex'
                assert local_cli.is_file()
                assert (local_cli.resolve().parent / 'codex-code-mode-host').is_file()
                assert (local_cli.resolve().parent.parent / 'codex-resources/fixture').is_file()
                apt_args = log.read_text()
                assert '--no-download' in apt_args and '--no-remove' in apt_args
                assert 'Dir::State::lists=/dev/null' in apt_args and 'update\n' not in apt_args
                config = installed / 'config/config.json'
                config.write_text('{"apiKey":"keep-private-fixture"}')
                existing_cli = local_cli.resolve()
                subprocess.run(['bash', str(bundle / 'install.sh')], env=env, check=True)
                assert local_cli.resolve() == existing_cli
                assert config.read_text() == '{"apiKey":"keep-private-fixture"}'
                # Build a real .deb from the full fixture without network or Rust.
                # Old full archives also need filtering when reused via --from-bundle.
                legacy = root / 'legacy/lexington'
                shutil.copytree(bundle, legacy)
                stale_windows = legacy / 'resource/runtime/windows-x86_64'
                stale_windows.mkdir(parents=True, exist_ok=True)
                (stale_windows / 'onnxruntime.dll').write_text('legacy Windows DLL')
                stale_drafts = legacy / 'resource/illustration/lexington/live2d-drafts'
                stale_drafts.mkdir(parents=True)
                (stale_drafts / 'preview.gif').write_text('legacy draft')
                with (legacy / 'SHA256SUMS').open('a') as sums:
                    for path in (stale_windows / 'onnxruntime.dll', stale_drafts / 'preview.gif'):
                        sums.write(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.relative_to(legacy).as_posix() + '\n')
                legacy_archive = root / 'legacy-full.tar.gz'
                with tarfile.open(legacy_archive, 'w:gz') as package:
                    package.add(legacy, arcname='lexington')
                with patch.object(deb_builder, '__file__', str(scripts / 'package-deb.py')), \
                     patch.object(deb_builder.dependencies, 'fetch', side_effect=AssertionError('DEB reuse must be offline')):
                    deb_output = output / 'deb'
                    deb_builder.build(deb_output, legacy_archive)
                deb = next(deb_output.glob('lexington_*.deb'))
                control = subprocess.check_output(['/usr/bin/dpkg-deb', '-f', str(deb)], text=True)
                assert 'Package: lexington\n' in control and 'python3 (>= 3.9)' in control
                assert 'libstdc++6' in control
                deb_root = root / 'extracted deb'
                subprocess.run(['/usr/bin/dpkg-deb', '--raw-extract', str(deb), str(deb_root)], check=True)
                filesystem = subprocess.check_output(['/usr/bin/dpkg-deb', '--fsys-tarfile', str(deb)])
                with tarfile.open(fileobj=io.BytesIO(filesystem)) as package:
                    assert all(member.uid == 0 and member.gid == 0 for member in package.getmembers())
                payload = deb_root / 'usr/lib/lexington'
                assert not (payload / 'offline/debs').exists()
                assert not (payload / 'resource/runtime/windows-x86_64').exists()
                assert not (payload / 'resource/illustration/lexington/live2d-drafts').exists()
                assert sorted(p.name for p in (payload / 'config').iterdir()) == ['config.json']
                assert (payload / 'config/config.json').read_text() == '{}\n'
                for name in ('preinst', 'postinst', 'postrm'):
                    script = (deb_root / 'DEBIAN' / name).read_text()
                    assert 'apt-get' not in script and 'sudo' not in script and 'HOME' not in script
                    subprocess.run(['sh', '-n', str(deb_root / 'DEBIAN' / name)], check=True)
                # First launch migrates this user's install without writing system paths.
                before_tree = sorted(str(p.relative_to(payload)) for p in payload.rglob('*'))
                before_apt = log.read_bytes()
                launcher = payload / 'scripts/launch-deb.sh'
                subprocess.run(['bash', str(launcher), '--fixture', 'value with spaces'], env=env, check=True)
                assert (root / 'launch.log').read_text().splitlines() == ['--fixture', 'value with spaces']
                marker = installed / '.deb-package-id'
                assert marker.read_bytes() == (payload / 'package-id').read_bytes()
                assert config.read_text() == '{"apiKey":"keep-private-fixture"}'
                assert log.read_bytes() == before_apt
                assert sorted(str(p.relative_to(payload)) for p in payload.rglob('*')) == before_tree
                assert 'Exec="/usr/bin/lexington"' in (home / 'Desktop/Lexington.desktop').read_text()
                marker_time = marker.stat().st_mtime_ns
                subprocess.run(['bash', str(launcher), '--second-launch'], env=env, check=True)
                assert marker.stat().st_mtime_ns == marker_time
                (payload / 'package-id').write_text('updated-package-content\n')
                subprocess.run(['bash', str(launcher), '--after-upgrade'], env=env, check=True)
                assert marker.read_text() == 'updated-package-content\n'
                assert config.read_text() == '{"apiKey":"keep-private-fixture"}'
                assert log.read_bytes() == before_apt
                # The offline installer passes the app and dependency DEBs to APT together.
                kit_archive = next(deb_output.glob('lexington-deb-offline-*.tar.gz'))
                kit_root = root / 'offline deb kit'
                with tarfile.open(kit_archive) as package:
                    package.extractall(kit_root, filter='data')
                kit = kit_root / 'lexington-deb'
                subprocess.run(['bash', str(kit / 'install.sh')], env=env, check=True)
                assert str(kit / 'offline/debs' / deb.name) in log.read_text()
                assert str(kit / 'offline/debs/fixture.deb') in log.read_text()
                # Corruption and mismatched platforms must fail before privilege/package calls.
                log.unlink()
                deb = bundle / 'offline/debs/fixture.deb'
                deb.write_text('corrupt')
                command = ['bash', str(bundle / 'scripts/install-offline-dependencies.sh')]
                assert subprocess.run(command, env=env, capture_output=True).returncode != 0
                assert not log.exists()
                deb.write_text('fixture-deb')
                platform_file = bundle / 'offline/platform'
                platform_file.write_text('debian\n0\narm64\n')
                sums = bundle / 'SHA256SUMS'
                lines = sums.read_text().splitlines()
                sums.write_text('\n'.join(
                    hashlib.sha256(platform_file.read_bytes()).hexdigest() + '  offline/platform'
                    if line.endswith('  offline/platform') else line for line in lines) + '\n')
                result = subprocess.run(command, env=env, capture_output=True, text=True)
                assert result.returncode != 0 and 'does not match' in result.stderr
                assert not log.exists()
                # Failed rebuilds retain the last completed archive.
                before = filename.read_bytes()
                metadata['dist']['integrity'] = 'sha512-' + base64.b64encode(b'wrong').decode()
                with self.assertRaisesRegex(RuntimeError, '完整性校验失败'):
                    builder.build(output)
                assert filename.read_bytes() == before
                assert not list(output.glob('.lexington-full-*'))


if __name__ == '__main__':
    unittest.main()
