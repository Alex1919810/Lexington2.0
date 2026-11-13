#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
fixture_python="$(command -v python3)"
temporary_root="$(cd -- "${TMPDIR:-/tmp}" && pwd -P)"
fixture="$(mktemp -d "$temporary_root/lexington-package-test.XXXXXX")"
cleanup() {
    case "$fixture" in "$temporary_root"/lexington-package-test.*) rm -rf -- "$fixture" ;; esac
}
trap cleanup EXIT
mkdir -p "$fixture/scripts" "$fixture/bin" "$fixture/resource/agents" "$fixture/config" "$fixture/target/release" "$fixture/integrations" "$fixture/dist/lexington/config"
cp "$project_root/scripts/package.sh" "$project_root/scripts/package_assets.py" "$fixture/scripts/"
for name in install-linux.sh install-dependencies.sh dependencies.py integrations.py; do printf 'fixture\n' > "$fixture/scripts/$name"; done
printf 'fixture binary\n' > "$fixture/target/release/lexington"
printf 'fixture\n' > "$fixture/README.md"
printf 'default character\n' > "$fixture/resource/agents/fixture.md"
mkdir -p "$fixture/resource/illustration/lexington/live2d-drafts/private" "$fixture/resource/runtime/windows-x86_64" "$fixture/resource/runtime/linux-x86_64"
printf 'draft fixture\n' > "$fixture/resource/illustration/lexington/live2d-drafts/private/preview.gif"
printf 'Windows fixture\n' > "$fixture/resource/runtime/windows-x86_64/onnxruntime.dll"
printf 'Linux fixture\n' > "$fixture/resource/runtime/linux-x86_64/fixture.txt"
# A repeated package must also remove exclusions already present in generated output.
mkdir -p "$fixture/dist/lexington/resource/illustration/lexington/live2d-drafts" "$fixture/dist/lexington/resource/runtime/windows-x86_64"
printf 'stale draft\n' > "$fixture/dist/lexington/resource/illustration/lexington/live2d-drafts/stale.gif"
printf 'stale Windows fixture\n' > "$fixture/dist/lexington/resource/runtime/windows-x86_64/stale.dll"
mkdir -p "$fixture/resource/models/bge-small-zh-v1.5"
for name in model.onnx config.json tokenizer.json tokenizer_config.json special_tokens_map.json manifest.json LICENSE.txt; do
    printf 'fixture-model\n' > "$fixture/resource/models/bge-small-zh-v1.5/$name"
done
printf '{"apiKey":"fixture-private-config"}\n' > "$fixture/config/config.json"
printf 'fixture-private-card\n' > "$fixture/config/roleplay.json"
printf 'fixture-private-memory\n' > "$fixture/config/roleplay-memory.json"
cp "$fixture/config/roleplay.json" "$fixture/config/roleplay-memory.json" "$fixture/dist/lexington/config/"
for suffix in '' -journal -wal -shm; do
    printf 'fixture-private-vector\n' > "$fixture/dist/lexington/config/roleplay-vectors.sqlite3$suffix"
done
cat > "$fixture/bin/cargo" <<'CARGO'
#!/usr/bin/env bash
exit 0
CARGO
cat > "$fixture/bin/python3" <<'PYTHON'
#!/usr/bin/env bash
if [[ "$1" == scripts/package_assets.py ]]; then exec "$LEXINGTON_TEST_PYTHON" "$@"; fi
mkdir -p dist/integrations
printf 'fixture extension\n' > dist/integrations/fixture.txt
PYTHON
cat > "$fixture/bin/strip" <<'STRIP'
#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == --strip-all && "$2" == -o ]]
printf '%s\n' "$@" >> "$LEXINGTON_TEST_STRIP_LOG"
cp -- "$4" "$3"
STRIP
chmod +x "$fixture/bin/cargo" "$fixture/bin/python3" "$fixture/bin/strip"
export LEXINGTON_TEST_PYTHON="$fixture_python" LEXINGTON_TEST_STRIP_LOG="$fixture/strip.log"
PATH="$fixture/bin:$PATH" bash "$fixture/scripts/package.sh" >/dev/null
bundle="$fixture/dist/lexington"
mkdir -p "$bundle/config/characters/private/png"
printf 'private artwork\n' > "$bundle/config/characters/private/png/normal.png"
PATH="$fixture/bin:$PATH" bash "$fixture/scripts/package.sh" >/dev/null
[[ ! -e "$bundle/config/characters" ]]
[[ "$(cat "$bundle/config/config.json")" == '{}' ]]
[[ ! -e "$bundle/config/roleplay.json" && ! -e "$bundle/config/roleplay-memory.json" ]]
for suffix in '' -journal -wal -shm; do
    [[ ! -e "$bundle/config/roleplay-vectors.sqlite3$suffix" ]]
done
[[ -f "$bundle/resource/agents/fixture.md" ]]
[[ ! -e "$bundle/resource/illustration/lexington/live2d-drafts" && ! -e "$bundle/resource/runtime/windows-x86_64" ]]
[[ -f "$bundle/resource/runtime/linux-x86_64/fixture.txt" ]]
[[ -f "$fixture/resource/illustration/lexington/live2d-drafts/private/preview.gif" && -f "$fixture/resource/runtime/windows-x86_64/onnxruntime.dll" ]]
[[ "$(cat "$fixture/target/release/lexington")" == 'fixture binary' ]]
[[ "$(wc -l < "$fixture/strip.log")" -eq 8 ]]
[[ -f "$bundle/resource/models/bge-small-zh-v1.5/model.onnx" ]]
[[ -f "$fixture/config/roleplay-memory.json" ]]
archive="$fixture/dist/lexington-linux-$(uname -m).tar.gz"
[[ -f "$archive" ]]
listing="$(tar -tzf "$archive")"
[[ "$listing" != *'config/roleplay.json'* && "$listing" != *'config/roleplay-memory.json'* ]]
[[ "$listing" != *'roleplay-vectors.sqlite3'* ]]
[[ "$listing" != *'config/characters/'* ]]
[[ "$listing" != *'live2d-drafts/'* && "$listing" != *'runtime/windows-'* ]]
[[ "$listing" == *'resource/models/bge-small-zh-v1.5/model.onnx'* ]]
rm "$fixture/resource/models/bge-small-zh-v1.5/model.onnx"
if PATH="$fixture/bin:$PATH" bash "$fixture/scripts/package.sh" >/dev/null 2>&1; then
    printf 'Packaging accepted missing embedding model.\n' >&2
    exit 1
fi
printf 'Linux bundle privacy and role-resource fixture checks passed.\n'
