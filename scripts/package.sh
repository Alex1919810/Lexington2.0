#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
cargo build --release --locked
bundle="$project_root/dist/lexington"
mkdir -p "$bundle"
cp target/release/lexington "$bundle/lexington"
cp -R resource "$bundle/"
mkdir -p "$bundle/config"
printf '{}\n' > "$bundle/config/config.json"
# Generated bundles must never retain private role cards or memories from an older run.
rm -f "$bundle/config/roleplay.json" "$bundle/config/roleplay-memory.json"
cp README.md "$bundle/README.md"
python3 scripts/integrations.py build --output "$project_root/dist/integrations"
cp -R integrations "$bundle/"
mkdir -p "$bundle/integrations/artifacts"
cp -R "$project_root/dist/integrations/." "$bundle/integrations/artifacts/"
mkdir -p "$bundle/scripts"
cp scripts/install-linux.sh scripts/install-dependencies.sh scripts/dependencies.py scripts/integrations.py "$bundle/scripts/"
cat > "$bundle/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
exec bash "$(dirname -- "${BASH_SOURCE[0]}")/scripts/install-linux.sh" "$@"
INSTALL
chmod +x "$bundle/install.sh"
tar -C "$project_root/dist" -czf "$project_root/dist/lexington-linux-$(uname -m).tar.gz" lexington
printf 'Bundle: %s\n' "$bundle"
