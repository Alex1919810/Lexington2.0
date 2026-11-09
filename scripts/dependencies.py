#!/usr/bin/env python3
"""Install missing Linux dependencies; retain existing CLI/toolchain and user config."""
import argparse
import base64
import hashlib
import http.client
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.parse
import urllib.request

# Keep the default in sync with the app-server protocol tested by Lexington.
CODEX_VERSION = '0.166.0'


def fetch(url, target=None):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != 'https' or parsed.hostname not in ('registry.npmjs.org', 'static.rust-lang.org'):
        raise RuntimeError('拒绝非官方依赖下载地址')
    request = urllib.request.Request(url, headers={'User-Agent': 'Lexington-installer'})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                if target is None:
                    return response.read()
                with Path(target).open('wb') as output:
                    shutil.copyfileobj(response, output)
                return None
        except (OSError, ValueError, http.client.HTTPException):
            if attempt == 2:
                raise


def system_packages(optional, build=True):
    required = []
    extras = []
    if build and (not shutil.which('cc') or not shutil.which('make')):
        required.append('build-essential')
    if not shutil.which('xdg-user-dir'):
        required.append('xdg-user-dirs')
    # rfd uses the desktop portal first, then Zenity if no FileChooser backend exists.
    if not shutil.which('zenity'):
        required.append('zenity')
    if shutil.which('dpkg-query'):
        packages = ['ca-certificates', 'libgl1', 'libegl1', 'libx11-6', 'libxcb1',
                    'libxkbcommon0', 'libxkbcommon-x11-0', 'libdbus-1-3', 'libstdc++6']
        for package in packages:
            result = subprocess.run(['dpkg-query', '-W', '-f=${db:Status-Status}', package], capture_output=True, text=True)
            if result.returncode or result.stdout != 'installed':
                required.append(package)
        if optional and shutil.which('nautilus'):
            result = subprocess.run(['dpkg-query', '-W', '-f=${db:Status-Status}', 'python3-nautilus'], capture_output=True, text=True)
            if result.returncode or result.stdout != 'installed':
                extras.append('python3-nautilus')
    if optional:
        if not shutil.which('gdbus') or not shutil.which('gio'):
            extras.append('libglib2.0-bin')
        if not shutil.which('notify-send'):
            extras.append('libnotify-bin')
    packages = required + extras
    if not packages:
        print('系统依赖已就绪。', flush=True)
        return
    print('安装缺少的系统依赖：' + ', '.join(packages), flush=True)
    apt = shutil.which('apt-get')
    if not apt:
        if required:
            raise RuntimeError('当前发行版未自动适配，请先安装：' + ', '.join(required))
        print('可选桌面依赖需由发行版包管理器安装：' + ', '.join(extras), flush=True)
        return
    prefix = []
    if os.geteuid() != 0:
        sudo = shutil.which('sudo')
        if sudo and subprocess.run([sudo, '-n', 'true'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
            prefix = [sudo, '-n']
        elif sys.stdin.isatty() and sudo:
            prefix = [sudo]
        elif shutil.which('pkexec') and os.environ.get('DBUS_SESSION_BUS_ADDRESS'):
            prefix = [shutil.which('pkexec')]
        else:
            if required:
                raise RuntimeError('系统依赖需要管理员权限：sudo apt-get install ' + ' '.join(packages))
            print('无法取得管理员权限；保留脚本菜单，可选顶层菜单需安装：' + ', '.join(extras), flush=True)
            return
    result = subprocess.run(prefix + [apt, 'install', '-y', *packages], check=False)
    if result.returncode:
        if required:
            raise RuntimeError('系统依赖安装失败或授权已取消，请处理后重新运行安装脚本。')
        print('可选桌面依赖未安装；继续安装，Nautilus 仍提供“脚本”入口。', flush=True)


def install_rust():
    if shutil.which('cargo') and shutil.which('rustc'):
        print('Rust 已安装，保留现有工具链。', flush=True)
        return
    machine = {'x86_64': 'x86_64', 'aarch64': 'aarch64', 'arm64': 'aarch64'}.get(platform.machine())
    if not machine:
        raise RuntimeError('当前架构需先自行安装 Rust 稳定版。')
    url = f'https://static.rust-lang.org/rustup/dist/{machine}-unknown-linux-gnu/rustup-init'
    with tempfile.TemporaryDirectory(prefix='lexington-rust-') as temporary:
        target = Path(temporary) / 'rustup-init'
        fetch(url, target)
        expected = fetch(url + '.sha256').decode().split()[0]
        if hashlib.sha256(target.read_bytes()).hexdigest() != expected:
            raise RuntimeError('Rust 安装器校验失败')
        target.chmod(0o700)
        subprocess.run([str(target), '-y', '--profile', 'minimal', '--no-modify-path', '--default-toolchain', 'stable'], check=True)


def install_codex():
    candidate = shutil.which('codex')
    if candidate:
        result = subprocess.run([candidate, '--version'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15)
        if result.returncode == 0:
            print('Codex 已安装，保留现有版本与登录配置。', flush=True)
            return
        raise RuntimeError('现有 codex 无法运行，请修复后重试；安装器不会覆盖已有命令。')
    if os.path.lexists(Path.home() / '.local/bin/codex'):
        raise RuntimeError('~/.local/bin/codex 已存在但不可执行，请修复后重试；未覆盖现有文件。')
    arch = {'x86_64': ('x64', 'x86_64'), 'aarch64': ('arm64', 'aarch64'), 'arm64': ('arm64', 'aarch64')}.get(platform.machine())
    if not arch:
        raise RuntimeError('当前架构需自行安装 Codex CLI。')
    metadata = json.loads(fetch(f'https://registry.npmjs.org/@openai/codex/{CODEX_VERSION}-linux-{arch[0]}'))
    if metadata.get('name') != '@openai/codex' or metadata.get('version') != f'{CODEX_VERSION}-linux-{arch[0]}':
        raise RuntimeError('Codex 官方包元数据不匹配')
    distribution = metadata['dist']
    integrity = distribution['integrity']
    if not integrity.startswith('sha512-'):
        raise RuntimeError('Codex 官方包缺少 SHA-512 校验')
    destination = Path.home() / '.local/share/lexington/dependencies'
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.codex-download-', dir=destination) as temporary:
        temporary = Path(temporary)
        archive = temporary / 'codex.tgz'
        print(f'下载官方 Codex {CODEX_VERSION} 原生包（无需 Node/npm）…', flush=True)
        fetch(distribution['tarball'], archive)
        digest = hashlib.sha512()
        with archive.open('rb') as content:
            for chunk in iter(lambda: content.read(1024 * 1024), b''):
                digest.update(chunk)
        if digest.digest() != base64.b64decode(integrity.removeprefix('sha512-'), validate=True):
            raise RuntimeError('Codex 包完整性校验失败，未安装')
        unpacked = temporary / 'unpacked'
        unpacked.mkdir()
        with tarfile.open(archive, 'r:gz') as package:
            if hasattr(tarfile, 'data_filter'):
                package.extractall(unpacked, filter='data')
            else:
                for member in package.getmembers():
                    target = (unpacked / member.name).resolve()
                    if not target.is_relative_to(unpacked.resolve()) or not (member.isfile() or member.isdir()):
                        raise RuntimeError('Codex 包包含不安全路径')
                package.extractall(unpacked)
        vendor = unpacked / f'package/vendor/{arch[1]}-unknown-linux-musl'
        executable = vendor / 'bin/codex'
        if not executable.is_file() or not (vendor / 'codex-package.json').is_file():
            raise RuntimeError('Codex 原生包结构不匹配')
        executable.chmod(0o755)
        subprocess.run([str(executable), '--version'], check=True, timeout=15)
        # Preserve all sibling resources/helpers needed by the native distribution.
        installed = destination / f'codex-{CODEX_VERSION}-{arch[1]}-{os.getpid()}'
        vendor.rename(installed)
        bin_directory = Path.home() / '.local/bin'
        bin_directory.mkdir(parents=True, exist_ok=True)
        link = bin_directory / 'codex'
        try:
            link.symlink_to(installed / 'bin/codex')
        except FileExistsError:
            print('检测到并发安装的 codex，保留现有命令。', flush=True)
    print('Codex 已安装到 ~/.local/bin/codex；登录可运行 codex login，或在 Lexington 配置模型 API。', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-optional', action='store_true', help='跳过文件管理器顶层菜单等可选系统依赖')
    parser.add_argument('--codex-only', action='store_true', help='仅检查/安装 Codex')
    parser.add_argument('--runtime-only', action='store_true', help='安装发布包时跳过编译工具链')
    options = parser.parse_args()
    try:
        if platform.system() != 'Linux':
            raise RuntimeError('自动依赖安装目前仅支持 Linux。')
        if not options.codex_only:
            system_packages(not options.skip_optional, build=not options.runtime_only)
            if not options.runtime_only:
                install_rust()
        install_codex()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError, tarfile.TarError, http.client.HTTPException) as error:
        print(f'依赖安装失败：{error}', file=sys.stderr)
        sys.exit(1)
