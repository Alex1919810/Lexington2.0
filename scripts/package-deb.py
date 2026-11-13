#!/usr/bin/env python3
"""Create a standard Debian package and a complete offline APT installation kit."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile

import dependencies
from package_assets import copy_linux_resources, strip_linux_binary


def digest(path, algorithm='sha256'):
    result = hashlib.new(algorithm)
    with path.open('rb') as content:
        for chunk in iter(lambda: content.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def build(output_directory, from_bundle=None, codex_version=None):
    root = Path(__file__).resolve().parent.parent
    if platform.system() != 'Linux' or not shutil.which('dpkg-deb'):
        raise RuntimeError('DEB 打包需要 Debian/Ubuntu Linux 和 dpkg-deb。')
    release = platform.freedesktop_os_release()
    distro, version = release.get('ID', ''), release.get('VERSION_ID', '')
    arch = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
    if distro not in ('ubuntu', 'debian') or not re.fullmatch(r'[0-9.]+', version) or arch not in ('amd64', 'arm64'):
        raise RuntimeError('DEB 打包仅支持 Debian/Ubuntu amd64/arm64。')
    app_version = re.search(r'^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"',
                            (root / 'Cargo.toml').read_text(), re.MULTILINE)
    if not app_version:
        raise RuntimeError('无法读取 Cargo.toml 中的应用版本。')
    if from_bundle is not None and codex_version is not None:
        raise RuntimeError('--from-bundle 使用包内 Codex，不能同时指定 --codex-version。')
    output_directory.mkdir(parents=True, exist_ok=True)
    if from_bundle is None:
        command = ['bash', str(root / 'scripts/package-full.sh'), '--output-dir', str(output_directory)]
        if codex_version is not None:
            command += ['--codex-version', codex_version]
        subprocess.run(command, check=True)
        from_bundle = output_directory / f'lexington-full-{distro}-{version}-{arch}.tar.gz'
    with tempfile.TemporaryDirectory(prefix='.lexington-deb-', dir=output_directory) as temporary:
        temporary = Path(temporary)
        unpacked = temporary / 'unpacked'
        unpacked.mkdir()
        with tarfile.open(from_bundle, 'r:gz') as archive:
            # Full bundles contain regular files/directories only, never symlinks/devices.
            for member in archive.getmembers():
                if not (member.isfile() or member.isdir()) or not (unpacked / member.name).resolve().is_relative_to(unpacked):
                    raise RuntimeError('输入安装包包含不安全路径或特殊文件。')
            if hasattr(tarfile, 'data_filter'):
                archive.extractall(unpacked, filter='data')
            else:
                archive.extractall(unpacked)
        bundle = unpacked / 'lexington'
        # Validate input as data; do not execute installation scripts from the archive.
        for line in (bundle / 'SHA256SUMS').read_text().splitlines():
            checksum, name = line.split('  ', 1)
            path = bundle / name
            if not path.resolve().is_relative_to(bundle) or not path.is_file() or digest(path) != checksum:
                raise RuntimeError('输入安装包文件完整性校验失败。')
        if (bundle / 'offline/platform').read_text().splitlines() != [distro, version, arch]:
            raise RuntimeError('输入安装包与制作机平台不匹配。')
        manifest = json.loads((bundle / 'offline/manifest.json').read_text())
        if (manifest['distribution'], manifest['version'], manifest['architecture']) != (distro, version, arch):
            raise RuntimeError('输入安装包与制作机平台不匹配。')
        dependencies.codex_metadata({'amd64': 'x64', 'arm64': 'arm64'}[arch], bundle / 'offline')
        revision = datetime.now(timezone.utc).strftime('%Y%m%d%H%M%S')
        deb_version = f'{app_version[1]}-1+{distro}{version}.{revision}'
        deb_name = f'lexington_{deb_version}_{arch}.deb'
        package = temporary / 'package'
        payload = package / 'usr/lib/lexington'
        payload.mkdir(parents=True)
        strip_linux_binary(bundle / 'lexington', payload / 'lexington')
        copy_linux_resources(bundle / 'resource', payload / 'resource')
        shutil.copytree(bundle / 'integrations', payload / 'integrations')
        (payload / 'config').mkdir()
        (payload / 'config/config.json').write_text('{}\n')
        (payload / 'scripts').mkdir()
        for name in ('install-linux.sh', 'dependencies.py', 'integrations.py', 'launch-deb.sh'):
            shutil.copy2(root / 'scripts' / name, payload / 'scripts' / name)
        (payload / 'offline').mkdir()
        for name in ('codex.json', 'codex.tgz', 'manifest.json', 'platform'):
            shutil.copy2(bundle / 'offline' / name, payload / 'offline' / name)
        # Content identity also changes for resource/script updates with the same app version.
        identity = hashlib.sha256()
        for path in sorted(payload.rglob('*')):
            if path.is_file():
                identity.update(path.relative_to(payload).as_posix().encode() + b'\0')
                identity.update(digest(path).encode())
        (payload / 'package-id').write_text(identity.hexdigest() + '\n')
        executable = package / 'usr/bin/lexington'
        executable.parent.mkdir(parents=True)
        executable.write_text('#!/bin/sh\nexec /bin/bash /usr/lib/lexington/scripts/launch-deb.sh "$@"\n')
        executable.chmod(0o755)
        desktop = package / 'usr/share/applications/lexington.desktop'
        desktop.parent.mkdir(parents=True)
        desktop.write_text(
            '[Desktop Entry]\nType=Application\nName=Lexington\nComment=Rust 桌面宠物\n'
            'Exec=/usr/bin/lexington\nIcon=/usr/lib/lexington/resource/icon/Normal.ico\n'
            'Terminal=false\nCategories=Game;Utility;\nStartupWMClass=lexington\n')
        docs = package / 'usr/share/doc/lexington'
        docs.mkdir(parents=True)
        shutil.copy2(root / 'README.md', docs / 'README.md')
        (docs / 'copyright').write_text(
            'Lexington\nLicense declared by Cargo.toml: MIT\n'
            'Bundled model/font/runtime license notices are shipped with their assets.\n'
            'Codex is distributed in its unmodified official archive with its notices.\n')
        control = package / 'DEBIAN'
        control.mkdir()
        runtime_packages = manifest['runtimePackages']
        if not runtime_packages or any(not re.fullmatch(r'[a-z0-9][a-z0-9+.-]+', name) for name in runtime_packages):
            raise RuntimeError('安装包包含无效的运行依赖名称。')
        depends = [name for name in runtime_packages if name != 'python3'] + ['python3 (>= 3.9)']
        installed_size = sum((p.stat().st_size + 1023) // 1024 for p in package.rglob('*') if p.is_file())
        (control / 'control').write_text(
            f'Package: lexington\nVersion: {deb_version}\nArchitecture: {arch}\n'
            'Section: games\nPriority: optional\nMaintainer: Lexington packaging <lexington@localhost>\n'
            f'Installed-Size: {installed_size}\nDepends: {", ".join(depends)}\n'
            'Description: Lexington native desktop companion\n'
            ' Includes artwork, the local embedding model and native Codex CLI.\n'
            ' User configuration and desktop integrations are initialized on first launch.\n')
        (control / 'preinst').write_text(
            '#!/bin/sh\nset -eu\ncase "${1:-}" in install|upgrade)\n'
            '. /etc/os-release\n'
            f'[ "${{ID:-}}" = "{distro}" ] && [ "${{VERSION_ID:-}}" = "{version}" ] || '
            '{ echo "Lexington package requires the matching distribution release." >&2; exit 1; }\n'
            ';; esac\nexit 0\n')
        # Maintainer scripts only refresh system caches, never call APT or touch user homes.
        for name in ('postinst', 'postrm'):
            (control / name).write_text(
                '#!/bin/sh\nset -eu\n'
                'if command -v update-desktop-database >/dev/null 2>&1; then\n'
                '    update-desktop-database /usr/share/applications\nfi\nexit 0\n')
        for name in ('preinst', 'postinst', 'postrm'):
            (control / name).chmod(0o755)
        md5sums = []
        for path in sorted((package / 'usr').rglob('*')):
            if path.is_file():
                md5sums.append(f'{digest(path, "md5")}  {path.relative_to(package).as_posix()}\n')
        (control / 'md5sums').write_text(''.join(md5sums))
        # dpkg-deb needs traversable control directories even with a restrictive umask.
        package.chmod(0o755)
        for path in package.rglob('*'):
            if path.is_dir():
                path.chmod(0o755)
            elif not path.stat().st_mode & 0o111:
                path.chmod(0o644)
        deb = temporary / deb_name
        subprocess.run(['dpkg-deb', '--root-owner-group', '-Zzstd', '-z6', '--threads-max=2', '--build', str(package), str(deb)], check=True)
        kit = temporary / 'lexington-deb'
        (kit / 'offline').mkdir(parents=True)
        shutil.copytree(bundle / 'offline/debs', kit / 'offline/debs')
        shutil.copy2(deb, kit / 'offline/debs' / deb_name)
        shutil.copy2(bundle / 'offline/platform', kit / 'offline/platform')
        manifest['applicationPackage'] = deb_name
        manifest['applicationVersion'] = deb_version
        (kit / 'offline/manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        (kit / 'scripts').mkdir()
        shutil.copy2(root / 'scripts/install-offline-dependencies.sh', kit / 'scripts')
        (kit / 'install.sh').write_text(
            '#!/usr/bin/env bash\nset -euo pipefail\n'
            'exec bash "$(dirname -- "${BASH_SOURCE[0]}")/scripts/install-offline-dependencies.sh" "$@"\n')
        (kit / 'install.sh').chmod(0o755)
        (kit / 'README.txt').write_text(
            f'Lexington DEB 离线安装：{distro} {version} {arch}\n'
            '执行 bash install.sh，一次安装程序 DEB 和全部本地系统依赖；需管理员认证。\n'
            '执行 bash install.sh --check，仅校验完整性和平台。\n'
            '安装后从应用菜单启动 Lexington；首次启动同步个人安装、桌面快捷方式及集成。\n'
            '已有配置、角色卡与 Codex 登录保留；Codex 最低 0.166.0，旧版需先升级。\n'
            '卸载系统 DEB 保留个人数据与个人安装，不遍历或清理用户目录。\n')
        checksums = []
        for path in sorted(kit.rglob('*')):
            if path.is_file():
                checksums.append(f'{digest(path)}  {path.relative_to(kit).as_posix()}\n')
        (kit / 'SHA256SUMS').write_text(''.join(checksums))
        subprocess.run(['bash', str(kit / 'install.sh'), '--check'], check=True)
        kit_name = f'lexington-deb-offline-{distro}-{version}-{arch}.tar.gz'
        with tarfile.open(temporary / kit_name, 'w:gz') as archive:
            archive.add(kit, arcname='lexington-deb')
        os.replace(deb, output_directory / deb_name)
        os.replace(temporary / kit_name, output_directory / kit_name)
        print(f'DEB 程序包：{output_directory / deb_name}\n完整离线套件：{output_directory / kit_name}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, default=Path(__file__).resolve().parent.parent / 'dist')
    parser.add_argument('--from-bundle', type=Path, help='复用已有完整离线 tar.gz，不重新构建或下载依赖')
    parser.add_argument('--codex-version', help='默认使用最新稳定版，最低 0.166.0；不能与 --from-bundle 同用')
    options = parser.parse_args()
    try:
        build(options.output_dir.resolve(), options.from_bundle, options.codex_version)
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError, tarfile.TarError) as error:
        print(f'DEB 打包失败：{error}', file=sys.stderr)
        sys.exit(1)
