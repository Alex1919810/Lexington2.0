#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
cargo build --release --locked
bundle="$project_root/dist/lexington"
mkdir -p "$bundle"
cp target/release/lexington "$bundle/lexington"
cp -R resource config "$bundle/"
cp README.md "$bundle/README.md"
python3 scripts/integrations.py build --output "$project_root/dist/integrations"
cp -R integrations "$bundle/"
mkdir -p "$bundle/integrations/artifacts"
cp -R "$project_root/dist/integrations/." "$bundle/integrations/artifacts/"
tar -C "$project_root/dist" -czf "$project_root/dist/lexington-linux-$(uname -m).tar.gz" lexington
printf 'Bundle: %s\n' "$bundle"
