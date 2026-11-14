"""Stage the pinned official native Codex distribution on the build host."""

import argparse
import base64
import hashlib
from pathlib import Path, PurePosixPath
import re
import shutil
import tarfile
import tempfile
import urllib.request

from dependencies import fetch


VERSION = '0.167.0'
TARGET = 'x86_64-pc-windows-msvc'
INTEGRITY = 'duPCBwflc8qhQdLekbL8fDYF646tW4gj0/l9BDnsVhEzoqm6OwxMAJXDFwSOLUUJPCmjBeM+eHYAx4fP/hwfjg=='
LICENSE_SHA256 = 'd17f227e4df5da1600391338865ce0f3055211760a36688f816941d58232d8dc'
REQUIRED = [f'vendor/{TARGET}/{suffix}' for suffix in (
    'bin/codex.exe', 'bin/codex-code-mode-host.exe', 'codex-path/rg.exe',
    'codex-resources/codex-command-runner.exe', 'codex-resources/codex-windows-sandbox-setup.exe',
    'codex-package.json',
)] + ['package.json', 'README.md', 'LICENSE']


def checksum(path, algorithm='sha512'):
    digest = hashlib.new(algorithm)
    with path.open('rb') as content:
        for chunk in iter(lambda: content.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.digest()


def unpack(archive, destination):
    with tarfile.open(archive, 'r:gz') as source:
        members = source.getmembers()
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if (not path.parts or path.parts[0] != 'package' or path.is_absolute()
                    or '..' in path.parts or '\\' in member.name or ':' in member.name
                    or any(part.rstrip(' .') != part for part in path.parts)
                    or any(re.fullmatch(r'(?i:con|prn|aux|nul|com[1-9]|lpt[1-9])', part.split('.')[0])
                           for part in path.parts)
                    or not (member.isfile() or member.isdir())):
                raise RuntimeError('Unsafe Codex archive member')
            key = path.as_posix().casefold()
            if key in seen:
                raise RuntimeError('Duplicate Codex archive member')
            seen.add(key)
        for member in members:
            target = destination.joinpath(*PurePosixPath(member.name).parts[1:])
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as content, target.open('xb') as output:
                    shutil.copyfileobj(content, output)


def stage(destination, cache):
    if destination.exists():
        raise RuntimeError('Codex staging destination must not exist')
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / f'codex-{VERSION}-win32-x64.tgz'
    expected = base64.b64decode(INTEGRITY, validate=True)
    if not archive.is_file():
        with tempfile.TemporaryDirectory(prefix='download-', dir=cache) as temporary:
            downloaded = Path(temporary) / 'codex.tgz'
            fetch(f'https://registry.npmjs.org/@openai/codex/-/{archive.name}', downloaded)
            if checksum(downloaded) != expected:
                raise RuntimeError('Codex SHA-512 mismatch')
            downloaded.replace(archive)
    if checksum(archive) != expected:
        raise RuntimeError('Cached Codex SHA-512 mismatch; remove the invalid cache and retry')
    license_path = cache / f'LICENSE-{VERSION}'
    if not license_path.is_file():
        url = f'https://raw.githubusercontent.com/openai/codex/rust-v{VERSION}/LICENSE'
        with urllib.request.urlopen(url, timeout=30) as response:
            license_bytes = response.read()
        if hashlib.sha256(license_bytes).hexdigest() != LICENSE_SHA256:
            raise RuntimeError('Codex license SHA-256 mismatch')
        license_path.write_bytes(license_bytes)
    if checksum(license_path, 'sha256').hex() != LICENSE_SHA256:
        raise RuntimeError('Cached Codex license SHA-256 mismatch')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='codex-stage-', dir=destination.parent) as temporary:
        staged = Path(temporary) / 'payload'
        staged.mkdir()
        unpack(archive, staged)
        shutil.copyfile(license_path, staged / 'LICENSE')
        for relative in REQUIRED:
            if not (staged / relative).is_file():
                raise RuntimeError(f'Incomplete native Codex distribution: {relative}')
        staged.rename(destination)
    print(f'Bundled official native Codex {VERSION} (SHA-512 verified; no Node/npm).')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--cache', type=Path, default=Path('.tools/codex'))
    options = parser.parse_args()
    stage(options.destination, options.cache)
