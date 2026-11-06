#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

# Python is the bootstrap dependency; other downloads use its HTTPS client.
if ! command -v python3 >/dev/null; then
    if ! command -v apt-get >/dev/null; then
        printf '请先安装 Python 3，再重新运行安装脚本。\n' >&2
        exit 1
    fi
    if [[ "$EUID" -eq 0 ]]; then
        apt-get update
        apt-get install -y python3 ca-certificates
    elif command -v sudo >/dev/null; then
        sudo apt-get update
        sudo apt-get install -y python3 ca-certificates
    else
        pkexec apt-get install -y python3 ca-certificates
    fi
fi
python3 "$project_root/scripts/dependencies.py" "$@"
