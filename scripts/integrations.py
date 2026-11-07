#!/usr/bin/env python3
"""Build/install context-menu clients without npm or a JavaScript build chain."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import zipfile
import base64

ROOT = Path(__file__).resolve().parent.parent


def build(output):
    output.mkdir(parents=True, exist_ok=True)
    browser = ROOT / 'integrations/browser'
    for kind in ('firefox', 'chromium'):
        target = output / kind
        target.mkdir(exist_ok=True)
        shutil.copy2(browser / f'manifest.{kind}.json', target / 'manifest.json')
        shutil.copy2(browser / 'background.js', target / 'background.js')
    with zipfile.ZipFile(output / 'lexington-firefox.xpi', 'w', zipfile.ZIP_DEFLATED) as archive:
        for name in ('manifest.json', 'background.js'):
            archive.write(output / 'firefox' / name, name)
    metadata = json.loads((ROOT / 'integrations/vscode/package.json').read_text(encoding='utf-8'))
    version = metadata['version']
    manifest = f'''<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="lexington-context" Version="{version}" Publisher="lexington"/>
    <DisplayName>Lexington</DisplayName><Description xml:space="preserve">看看Lexington怎么说</Description>
    <Categories>Other</Categories>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="^1.85.0"/>
      <Property Id="Microsoft.VisualStudio.Code.ExtensionKind" Value="ui"/>
    </Properties>
  </Metadata>
  <Installation><InstallationTarget Id="Microsoft.VisualStudio.Code"/></Installation>
  <Dependencies/>
  <Assets><Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true"/></Assets>
</PackageManifest>'''
    with zipfile.ZipFile(output / 'lexington-context.vsix', 'w', zipfile.ZIP_DEFLATED) as archive:
        archive.writestr('extension.vsixmanifest', manifest)
        archive.writestr('[Content_Types].xml', '''<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="json" ContentType="application/json"/>
<Default Extension="js" ContentType="application/javascript"/>
<Default Extension="vsixmanifest" ContentType="text/xml"/>
</Types>''')
        for name in ('package.json', 'extension.js'):
            archive.write(ROOT / 'integrations/vscode' / name, f'extension/{name}')


def install(directory, artifacts):
    data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
    config_root = Path(os.environ.get('XDG_CONFIG_HOME', Path.home() / '.config'))
    shutil.copytree(ROOT / 'integrations', directory / 'integrations', dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns('__pycache__', '*.pyc'))
    shutil.copytree(artifacts, directory / 'integrations/artifacts', dirs_exist_ok=True)
    (directory / 'integrations.json').write_text(json.dumps({'executable': str(directory / 'lexington')}) + '\n')

    scripts = data / 'nautilus/scripts'
    scripts.mkdir(parents=True, exist_ok=True)
    launcher = scripts / '看看Lexington怎么说'
    shutil.copy2(ROOT / 'integrations/nautilus/send.py', launcher)
    launcher.chmod(0o755)
    extensions = data / 'nautilus-python/extensions'
    extensions.mkdir(parents=True, exist_ok=True)
    shutil.copy2(ROOT / 'integrations/nautilus/lexington_menu.py', extensions / 'lexington_menu.py')
    print('Nautilus: 右键 → 脚本 → 看看Lexington怎么说；安装 nautilus-python 后也提供顶层菜单。')

    host = directory / 'integrations/native-host'
    host.write_text('#!/bin/sh\nexec ' + shlex.quote(str(directory / 'lexington')) + ' --native-messaging "$@"\n')
    host.chmod(0o755)
    common = {
        'name': 'com.lexington.context', 'description': 'Lexington context menu bridge',
        'path': str(host), 'type': 'stdio',
    }
    firefox = dict(common, allowed_extensions=['lexington-context@lexington.local'])
    firefox_dirs = [Path.home() / '.mozilla/native-messaging-hosts']
    # Ubuntu Snap Firefox reads native-host registrations in its own home.
    snap_home = Path.home() / 'snap/firefox/common'
    if snap_home.is_dir():
        firefox_dirs.append(snap_home / '.mozilla/native-messaging-hosts')
    for target in firefox_dirs:
        target.mkdir(parents=True, exist_ok=True)
        (target / 'com.lexington.context.json').write_text(json.dumps(firefox, indent=2) + '\n')

    manifest = json.loads((artifacts / 'chromium/manifest.json').read_text())
    digest = hashlib.sha256(base64.b64decode(manifest['key'])).digest()[:16].hex()
    extension_id = ''.join(chr(ord('a') + int(char, 16)) for char in digest)
    chromium = dict(common, allowed_origins=[f'chrome-extension://{extension_id}/'])
    for browser in ('google-chrome', 'chromium', 'BraveSoftware/Brave-Browser', 'microsoft-edge'):
        target = config_root / browser / 'NativeMessagingHosts'
        target.mkdir(parents=True, exist_ok=True)
        (target / 'com.lexington.context.json').write_text(json.dumps(chromium, indent=2) + '\n')
    print(f'浏览器扩展: {directory / "integrations/artifacts"}（需在浏览器中加载；Firefox 正式安装需签名）。')
    vscode = shutil.which('code')
    if vscode:
        try:
            result = subprocess.run(
                [vscode, '--install-extension', str(artifacts / 'lexington-context.vsix'), '--force'],
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                text=True, timeout=60, check=False,
            )
            if result.returncode:
                print('VS Code 扩展未自动安装，可手动从 VSIX 安装：' + str(artifacts / 'lexington-context.vsix'))
                print(result.stderr.strip()[:1000])
            else:
                print('VS Code: 已安装“看看Lexington怎么说”，已打开的窗口可能需要重新加载。')
        except (OSError, subprocess.TimeoutExpired) as error:
            print(f'VS Code 扩展需手动安装：{error}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('build', 'install'))
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--directory', type=Path)
    args = parser.parse_args()
    if args.action == 'build':
        build(args.output.resolve())
    elif args.directory:
        install(args.directory.resolve(), args.output.resolve())
    else:
        parser.error('install requires --directory')
