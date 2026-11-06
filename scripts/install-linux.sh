#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"
binary_source="target/release/lexington"
if [[ -f Cargo.toml ]]; then
    bash scripts/install-dependencies.sh "$@"
    cargo build --release --locked
else
    bash scripts/install-dependencies.sh --runtime-only "$@"
    binary_source="lexington"
fi
install_dir="${XDG_DATA_HOME:-$HOME/.local/share}/lexington"
mkdir -p "$install_dir" "${XDG_DATA_HOME:-$HOME/.local/share}/applications"
# Replace atomically: truncating a running executable fails with ETXTBSY.
binary_tmp="$(mktemp "$install_dir/.lexington-install.XXXXXX")"
cp "$binary_source" "$binary_tmp"
chmod +x "$binary_tmp"
mv -f "$binary_tmp" "$install_dir/lexington"
cp -R resource "$install_dir/"
# Preserve a user's API key and custom configuration on subsequent installs.
if [[ ! -f "$install_dir/config/config.json" ]]; then cp -R config "$install_dir/"; fi
launcher="${XDG_DATA_HOME:-$HOME/.local/share}/applications/lexington.desktop"
cat > "$launcher" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Lexington
Comment=Rust 桌面宠物
Exec="$install_dir/lexington"
Path=$install_dir
Icon=$install_dir/resource/icon/Normal.ico
Terminal=false
Categories=Game;Utility;
StartupWMClass=lexington
DESKTOP
chmod +x "$launcher"
desktop_dir="$(xdg-user-dir DESKTOP)"
mkdir -p "$desktop_dir"
desktop_launcher="$desktop_dir/Lexington.desktop"
# Migrate the previous filename so upgrades do not leave duplicate shortcuts.
if [[ -f "$desktop_dir/lexington.desktop" ]]; then mv -f "$desktop_dir/lexington.desktop" "$desktop_launcher"; fi
cp "$launcher" "$desktop_launcher"
chmod +x "$desktop_launcher"
if command -v gio >/dev/null; then gio set "$desktop_launcher" metadata::trusted true 2>/dev/null || true; fi
if command -v update-desktop-database >/dev/null; then update-desktop-database "$(dirname "$launcher")"; fi
python3 scripts/integrations.py build --output "$project_root/dist/integrations"
python3 scripts/integrations.py install --output "$project_root/dist/integrations" --directory "$install_dir"
printf 'Installed: %s\nDesktop shortcut: %s\n' "$install_dir/lexington" "$desktop_launcher"
