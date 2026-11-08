#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
temporary_root="$(cd -- "${TMPDIR:-/tmp}" && pwd -P)"
fixture="$(mktemp -d "$temporary_root/lexington-package-test.XXXXXX")"
cleanup() {
    case "$fixture" in "$temporary_root"/lexington-package-test.*) rm -rf -- "$fixture" ;; esac
}
trap cleanup EXIT
mkdir -p "$fixture/scripts" "$fixture/bin" "$fixture/resource/agents" "$fixture/config" "$fixture/target/release" "$fixture/integrations" "$fixture/dist/lexington/config"
cp "$project_root/scripts/package.sh" "$fixture/scripts/"
for name in install-linux.sh install-dependencies.sh dependencies.py integrations.py; do printf 'fixture\n' > "$fixture/scripts/$name"; done
printf 'fixture binary\n' > "$fixture/target/release/lexington"
printf 'fixture\n' > "$fixture/README.md"
printf 'default character\n' > "$fixture/resource/agents/fixture.md"
printf '{"apiKey":"fixture-private-config"}\n' > "$fixture/config/config.json"
printf 'fixture-private-card\n' > "$fixture/config/roleplay.json"
printf 'fixture-private-memory\n' > "$fixture/config/roleplay-memory.json"
cp "$fixture/config/roleplay.json" "$fixture/config/roleplay-memory.json" "$fixture/dist/lexington/config/"
cat > "$fixture/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
exit 0
CARGO
cat > "$fixture/bin/python3" <<'PYTHON'
#!/usr/bin/env bash
mkdir -p dist/integrations
printf 'fixture extension\n' > dist/integrations/fixture.txt
PYTHON
chmod +x "$fixture/bin/cargo" "$fixture/bin/python3"
PATH="$fixture/bin:$PATH" bash "$fixture/scripts/package.sh" >/dev/null
bundle="$fixture/dist/lexington"
[[ "$(cat "$bundle/config/config.json")" == '{}' ]]
[[ ! -e "$bundle/config/roleplay.json" && ! -e "$bundle/config/roleplay-memory.json" ]]
[[ -f "$bundle/resource/agents/fixture.md" ]]
[[ -f "$fixture/config/roleplay-memory.json" ]]
archive="$fixture/dist/lexington-linux-$(uname -m).tar.gz"
[[ -f "$archive" ]]
listing="$(tar -tzf "$archive")"
[[ "$listing" != *'config/roleplay.json'* && "$listing" != *'config/roleplay-memory.json'* ]]
printf 'Linux bundle privacy and role-resource fixture checks passed.\n'
