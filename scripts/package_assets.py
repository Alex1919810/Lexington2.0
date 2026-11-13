#!/usr/bin/env python3
"""Shared Linux release staging: runtime resources and stripped executable copies."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def copy_linux_resources(source, destination):
    source, destination = Path(source).resolve(), Path(destination)
    target = destination.resolve()
    if destination.is_symlink() or target == source or target.is_relative_to(source) or source.is_relative_to(target):
        raise RuntimeError('Resource staging must be separate from source resources.')
    # Prune exclusions left in an older generated tree as well as filtering new copies.
    excluded = list(destination.rglob('live2d-drafts')) + list((destination / 'runtime').glob('windows-*'))
    for path in excluded:
        if path.is_symlink():
            path.unlink()
        elif path.exists():
            if not path.resolve().is_relative_to(target):
                raise RuntimeError('Refusing resource cleanup outside staging.')
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()
    shutil.copytree(source, destination, dirs_exist_ok=True, ignore=lambda directory, names: [
        name for name in names if name == 'live2d-drafts'
        or (Path(directory) == source / 'runtime' and name.startswith('windows-'))
    ])


def strip_linux_binary(source, destination):
    source, destination = Path(source), Path(destination)
    if source.resolve() == destination.resolve():
        raise RuntimeError('Strip only the packaged copy, never the build output.')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.lexington-strip-', dir=destination.parent) as temporary:
        stripped = Path(temporary) / 'lexington'
        subprocess.run(['strip', '--strip-all', '-o', str(stripped), str(source)], check=True)
        stripped.chmod(0o755)
        os.replace(stripped, destination)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('resources', 'binary'))
    parser.add_argument('source', type=Path)
    parser.add_argument('destination', type=Path)
    options = parser.parse_args()
    if options.action == 'resources':
        copy_linux_resources(options.source, options.destination)
    else:
        strip_linux_binary(options.source, options.destination)
