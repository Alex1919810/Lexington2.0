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
tar -C "$project_root/dist" -czf "$project_root/dist/lexington-linux-$(uname -m).tar.gz" lexington
printf 'Bundle: %s\n' "$bundle"
