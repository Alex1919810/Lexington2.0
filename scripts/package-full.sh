#!/usr/bin/env bash
# Build an offline runtime installer for this Linux distribution and architecture.
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != Linux ]]; then
    printf '%s\n' 'Full offline packaging currently requires Debian/Ubuntu Linux.' >&2
    exit 1
fi
exec python3 "$project_root/scripts/package-full.py" "$@"
