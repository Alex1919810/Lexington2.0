#!/usr/bin/env bash
# System-owned assets, with writable application state and integrations per user.
set -euo pipefail
if [[ "$EUID" -eq 0 ]]; then
    printf '%s\n' 'Launch Lexington as your desktop user, without sudo.' >&2
    exit 1
fi
package_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
install_dir="${XDG_DATA_HOME:-$HOME/.local/share}/lexington"
if [[ ! -x "$install_dir/lexington" || ! -f "$install_dir/.deb-package-id" ]] || ! cmp -s "$package_root/package-id" "$install_dir/.deb-package-id"; then
    if ! bash "$package_root/scripts/install-linux.sh" --system-package; then
        if command -v zenity >/dev/null; then
            zenity --error --text='Lexington 首次启动配置失败，请在终端运行 lexington 查看错误。' 2>/dev/null || true
        fi
        exit 1
    fi
    marker="$(mktemp "$install_dir/.deb-package-id.XXXXXX")"
    cp "$package_root/package-id" "$marker"
    mv -f "$marker" "$install_dir/.deb-package-id"
fi
exec "$install_dir/lexington" "$@"
