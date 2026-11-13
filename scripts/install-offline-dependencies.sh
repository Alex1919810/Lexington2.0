#!/usr/bin/env bash
# Bootstrap system dependencies without Python or network downloads.
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
check_only=false
for option in "$@"; do
    case "$option" in
        --check) check_only=true ;;
        --runtime-only|--skip-optional|--codex-only) ;;
        *) printf 'Unknown offline installation option: %s\n' "$option" >&2; exit 1 ;;
    esac
done
if [[ "$(uname -s)" != Linux ]]; then
    printf '%s\n' 'Offline installation requires Debian/Ubuntu Linux.' >&2
    exit 1
fi
cd "$project_root"
sha256sum --check --quiet SHA256SUMS
mapfile -t target_platform < offline/platform
# Read the host system file only; package metadata is never executed as shell code.
source /etc/os-release
if [[ "${#target_platform[@]}" -ne 3 || "${ID:-}" != "${target_platform[0]}" || "${VERSION_ID:-}" != "${target_platform[1]}" || "$(dpkg --print-architecture)" != "${target_platform[2]}" ]]; then
    printf '%s\n' 'Offline package distribution/version/architecture does not match this system.' >&2
    exit 1
fi
shopt -s nullglob
debs=("$project_root"/offline/debs/*.deb)
if [[ "${#debs[@]}" -eq 0 ]]; then
    printf '%s\n' 'Offline package contains no DEBs.' >&2
    exit 1
fi
if "$check_only"; then
    printf '%s\n' 'Offline package checksums and platform verified.'
    exit 0
fi
prefix=()
if [[ "$EUID" -ne 0 ]]; then
    if command -v sudo >/dev/null; then
        prefix=(sudo)
    elif command -v pkexec >/dev/null; then
        prefix=(pkexec)
    else
        printf '%s\n' 'Installing offline system dependencies requires sudo or pkexec.' >&2
        exit 1
    fi
fi
# Ignore online indexes: the resolver can use only installed packages and local DEBs.
# No removals, automatic downgrades, apt update or fallback downloads.
"${prefix[@]}" apt-get -y --no-download --no-remove --no-install-recommends \
    -o Dir::Etc::sourcelist=/dev/null -o Dir::Etc::sourceparts=- \
    -o Dir::State::lists=/dev/null install "${debs[@]}"
