#!/usr/bin/env python3
"""Nautilus script entry; also used by the optional top-level menu provider."""
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.parse import unquote, urlsplit

try:
    data = Path(os.environ.get('XDG_DATA_HOME', Path.home() / '.local/share'))
    config = json.loads((data / 'lexington/integrations.json').read_text())
    files = sys.argv[1:]
    if not files:
        for uri in os.environ.get('NAUTILUS_SCRIPT_SELECTED_URIS', '').splitlines():
            url = urlsplit(uri)
            if url.scheme != 'file' or url.netloc not in ('', 'localhost'):
                raise ValueError('暂仅支持本地文件')
            files.append(unquote(url.path))
    if not files:
        raise ValueError('请先选择文件')
    if len(files) > 32:
        raise ValueError('一次最多选择 32 个文件')
    result = subprocess.run(
        [config['executable'], '--ask-files', *files],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE, text=True, timeout=20, check=False,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or '发送失败')
except Exception as error:
    # Context-menu subprocesses have no terminal; report failures visibly.
    try:
        subprocess.run(['notify-send', 'Lexington', str(error)[:1000]], check=False, timeout=3)
    except (OSError, subprocess.TimeoutExpired):
        pass
    sys.exit(1)
