#!/usr/bin/env python3
"""Build an offline Debian/Ubuntu runtime bundle, without installing host packages."""
import argparse
import base64
import hashlib
import http.client
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

# Resolve against an empty dpkg status: installed host packages must also be shipped.
RUNTIME_PACKAGES = [
    'bash', 'coreutils', 'tar', 'gzip', 'python3', 'ca-certificates', 'libc6',
    'libgcc-s1', 'libstdc++6', 'libgl1', 'libegl1', 'libglx-mesa0',
    'libegl-mesa0', 'libgl1-mesa-dri', 'libx11-6', 'libxcb1',
    'libxcursor1', 'libxrandr2', 'libxi6', 'libxkbcommon0', 'libxkbcommon-x11-0',
    'libwayland-client0', 'libwayland-cursor0', 'libwayland-egl1', 'libdbus-1-3',
    'dbus-user-session', 'xdg-user-dirs', 'xdg-utils', 'xdg-desktop-portal',
    'xdg-desktop-portal-gtk', 'zenity', 'fonts-noto-cjk', 'libglib2.0-bin',
    'libnotify-bin', 'python3-nautilus', 'desktop-file-utils',
]


def file_digest(path, algorithm):
    digest = hashlib.new(algorithm)
    with path.open('rb') as content:
        for chunk in iter(lambda: content.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest


def build(output_directory, codex_version=None):
    root = Path(__file__).resolve().parent.parent
    if platform.system() != 'Linux':
        raise RuntimeError('完整离线打包目前仅支持 Debian/Ubuntu Linux。')
    release = platform.freedesktop_os_release()
    distro, version = release.get('ID', ''), release.get('VERSION_ID', '')
    if distro not in ('ubuntu', 'debian') or not re.fullmatch(r'[0-9.]+', version):
        raise RuntimeError('请在目标 Debian/Ubuntu 发行版上制作离线包。')
    for tool in ('apt-get', 'dpkg', 'dpkg-deb', 'cargo', 'bash', 'tar', 'sha256sum', 'strip'):
        if not shutil.which(tool):
            raise RuntimeError(f'制作机缺少工具：{tool}')
    arch = subprocess.check_output(['dpkg', '--print-architecture'], text=True).strip()
    codex_arch = {'amd64': ('x64', 'x86_64'), 'arm64': ('arm64', 'aarch64')}.get(arch)
    if codex_arch is None:
        raise RuntimeError('离线包目前仅支持 amd64/arm64。')
    # Resolve the stable version before the expensive build/download steps.
    metadata, codex_version = dependencies.codex_metadata(codex_arch[0], version=codex_version)
    subprocess.run(['bash', str(root / 'scripts/package.sh')], cwd=root, check=True)
    output_directory.mkdir(parents=True, exist_ok=True)
    # A fresh allowlisted staging tree cannot inherit user data from earlier bundles.
    with tempfile.TemporaryDirectory(prefix='.lexington-full-', dir=output_directory) as temporary:
        staging = Path(temporary) / 'lexington'
        staging.mkdir()
        base = root / 'dist/lexington'
        for name in ('lexington', 'README.md', 'install.sh'):
            shutil.copy2(base / name, staging / name)
        for name in ('resource', 'integrations', 'scripts'):
            shutil.copytree(base / name, staging / name)
        shutil.copy2(root / 'scripts/install-offline-dependencies.sh', staging / 'scripts')
        (staging / 'config').mkdir()
        (staging / 'config/config.json').write_text('{}\n', encoding='utf-8')
        offline = staging / 'offline'
        debs = offline / 'debs'
        (debs / 'partial').mkdir(parents=True)
        status = Path(temporary) / 'empty-status'
        status.touch()
        # APT verifies downloads using the configured authenticated repository indexes.
        # Downloads and state stay in staging; no sudo, apt update or host installation.
        apt_command = [
            'apt-get', '-y', '--download-only', '--no-install-recommends',
            '-o', f'Dir::State::status={status}',
            '-o', f'Dir::State::extended_states={temporary}/extended-states',
            '-o', f'Dir::Cache::archives={debs}/',
            '-o', f'Dir::Cache::pkgcache={temporary}/pkgcache.bin',
            '-o', f'Dir::Cache::srcpkgcache={temporary}/srcpkgcache.bin',
            '-o', 'Debug::NoLocking=true',
            'install', *RUNTIME_PACKAGES,
        ]
        print('下载完整系统运行依赖（包含制作机已安装的依赖）…', flush=True)
        subprocess.run(apt_command, check=True)
        packages = []
        for deb in sorted(debs.glob('*.deb')):
            fields = subprocess.check_output(
                ['dpkg-deb', '-f', str(deb), 'Package', 'Version', 'Architecture'], text=True)
            packages.append({'file': deb.name, 'control': fields.strip()})
        if not packages:
            raise RuntimeError('APT 没有产生任何 DEB；请先更新制作机的软件源索引后重试。')
        shutil.rmtree(debs / 'partial')
        (debs / 'lock').unlink(missing_ok=True)
        distribution = metadata['dist']
        dependencies.fetch(distribution['tarball'], offline / 'codex.tgz')
        digest = file_digest(offline / 'codex.tgz', 'sha512').digest()
        if digest != base64.b64decode(distribution['integrity'][7:], validate=True):
            raise RuntimeError('Codex 包完整性校验失败')
        # Store only public distribution fields, never local CLI/config/login data.
        (offline / 'codex.json').write_text(json.dumps({
            'name': metadata['name'], 'version': metadata['version'], 'dist': distribution,
        }, indent=2) + '\n', encoding='utf-8')
        (offline / 'platform').write_text(f'{distro}\n{version}\n{arch}\n', encoding='utf-8')
        (offline / 'manifest.json').write_text(json.dumps({
            'distribution': distro, 'version': version, 'architecture': arch,
            'codexVersion': codex_version, 'minimumCodexVersion': dependencies.MIN_CODEX_VERSION,
            'runtimePackages': RUNTIME_PACKAGES,
            'packages': packages,
        }, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        (staging / 'OFFLINE-INSTALL.txt').write_text(
            f'Lexington 完整离线安装包：{distro} {version} {arch}\n'
            '解压后以普通桌面用户执行：bash install.sh\n'
            '先校验包：bash scripts/install-offline-dependencies.sh --check\n'
            '系统依赖安装需要管理员认证，整个安装不下载依赖；已有 Codex、登录与配置保留。\n'
            '目标机需已有 Bash、coreutils、APT/dpkg 和可用图形桌面/显示驱动。\n'
            '包内含 Mesa 通用驱动；专有 GPU 驱动、内核与桌面环境由目标机提供。\n'
            '安装离线；AI 对话仍需登录/API 配置及模型网络连接。\n', encoding='utf-8')
        checksums = []
        for path in sorted(staging.rglob('*')):
            if path.is_symlink():
                raise RuntimeError('离线包中不允许符号链接')
            if path.is_file():
                relative = path.relative_to(staging).as_posix()
                if '\n' in relative or '\\' in relative:
                    raise RuntimeError('离线包文件名不受支持')
                checksum = file_digest(path, 'sha256').hexdigest()
                checksums.append(f'{checksum}  {relative}\n')
        (staging / 'SHA256SUMS').write_text(''.join(checksums), encoding='utf-8')
        subprocess.run(['bash', str(staging / 'scripts/install-offline-dependencies.sh'), '--check'], check=True)
        name = f'lexington-full-{distro}-{version}-{arch}.tar.gz'
        archive = Path(temporary) / name
        with tarfile.open(archive, 'w:gz') as output:
            output.add(staging, arcname='lexington')
        os.replace(archive, output_directory / name)
        print(f'完整离线安装包：{output_directory / name}', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-dir', type=Path, default=Path(__file__).resolve().parent.parent / 'dist',
                        help='输出目录（默认 dist）')
    parser.add_argument('--codex-version', help='指定 Codex 稳定版（最低 0.166.0），默认获取官方 latest')
    options = parser.parse_args()
    try:
        build(options.output_dir.resolve(), options.codex_version)
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError, http.client.HTTPException) as error:
        print(f'完整离线打包失败：{error}', file=sys.stderr)
        sys.exit(1)
