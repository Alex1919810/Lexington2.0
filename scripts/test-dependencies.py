#!/usr/bin/env python3
"""Dependency installer checks using local fixtures, no network or system changes."""
import base64
import hashlib
import importlib.util
import io
import json
import os
import subprocess
from pathlib import Path
import tarfile
import tempfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('dependencies', Path(__file__).with_name('dependencies.py'))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


def archive_fixture(unsafe=False):
    contents = io.BytesIO()
    with tarfile.open(fileobj=contents, mode='w:gz') as archive:
        for name, data in {
            'package/vendor/x86_64-unknown-linux-musl/bin/codex': b'#!/bin/sh\necho codex-cli-fixture\n',
            'package/vendor/x86_64-unknown-linux-musl/bin/codex-code-mode-host': b'fixture',
            'package/vendor/x86_64-unknown-linux-musl/codex-package.json': b'{"layoutVersion":1}',
            'package/vendor/x86_64-unknown-linux-musl/codex-resources/fixture': b'fixture',
        }.items():
            member = tarfile.TarInfo(name)
            member.size = len(data)
            member.mode = 0o755
            archive.addfile(member, io.BytesIO(data))
        if unsafe:
            member = tarfile.TarInfo('../../escaped')
            member.size = 1
            archive.addfile(member, io.BytesIO(b'x'))
    return contents.getvalue()


def fake_fetch(package, corrupt=False):
    def fetch(url, target=None):
        if target:
            Path(target).write_bytes(package)
            return
        assert url == 'https://registry.npmjs.org/@openai/codex/0.166.0-linux-x64'
        digest = hashlib.sha512(b'corrupt' if corrupt else package).digest()
        return json.dumps({
            'name': '@openai/codex', 'version': '0.166.0-linux-x64',
            'dist': {'tarball': 'https://registry.npmjs.org/fixture.tgz',
                     'integrity': 'sha512-' + base64.b64encode(digest).decode()},
        }).encode()
    return fetch


with tempfile.TemporaryDirectory(prefix='lexington-dependency-tests-') as temporary:
    root = Path(temporary)
    for kind in ('valid', 'corrupt', 'unsafe'):
        home = root / kind
        home.mkdir()
        config = home / '.codex/config.toml'
        config.parent.mkdir()
        config.write_text('# preserve existing user config\n')
        archive = archive_fixture(kind == 'unsafe')
        with patch.object(installer.Path, 'home', return_value=home), \
             patch.object(installer.platform, 'machine', return_value='x86_64'), \
             patch.object(installer.shutil, 'which', return_value=None), \
             patch.object(installer, 'fetch', side_effect=fake_fetch(archive, kind == 'corrupt')):
            if kind == 'valid':
                installer.install_codex()
                binary = home / '.local/bin/codex'
                assert binary.is_file()
                assert (binary.resolve().parent.parent / 'codex-resources/fixture').is_file()
                assert (binary.resolve().parent / 'codex-code-mode-host').is_file()
                with patch.object(installer.shutil, 'which', return_value=str(binary)), \
                     patch.object(installer, 'fetch', side_effect=AssertionError('must not download existing CLI')):
                    installer.install_codex()
            else:
                try:
                    installer.install_codex()
                except (RuntimeError, tarfile.TarError):
                    pass
                else:
                    raise AssertionError('unsafe or corrupt package must be rejected')
                assert not (home / '.local/bin/codex').exists()
        assert config.read_text() == '# preserve existing user config\n'
    assert not (root / 'escaped').exists()
    for url in ('http://registry.npmjs.org/pkg', 'https://example.com/pkg'):
        try:
            installer.fetch(url)
        except RuntimeError:
            pass
        else:
            raise AssertionError('untrusted download URL accepted')
# A minimal desktop still needs a picker when no portal FileChooser backend exists.
for build in (True, False):
    for picker_present in (True, False):
        calls = []

        def which(command):
            return None if command == 'zenity' and not picker_present else '/fixture/' + command

        def run(command, **kwargs):
            calls.append(command)
            missing = command[0] == 'dpkg-query' and command[-1] in ('libdbus-1-3', 'libstdc++6')
            return subprocess.CompletedProcess(command, 0, stdout=(
                'not-installed' if missing else 'installed'
            ))

        with patch.object(installer.shutil, 'which', side_effect=which), \
             patch.object(installer.subprocess, 'run', side_effect=run), \
             patch.object(installer.os, 'geteuid', return_value=0):
            installer.system_packages(False, build=build)
        install = next(command for command in calls if 'install' in command)
        assert 'libdbus-1-3' in install
        assert 'libstdc++6' in install
        assert ('zenity' in install) == (not picker_present)
        assert 'build-essential' not in install
print('Codex integrity/config preservation and source/runtime file-picker dependency checks passed.')
