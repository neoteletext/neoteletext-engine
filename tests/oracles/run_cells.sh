#!/usr/bin/env bash
# REQ-TST-023: decodes T42 files with neottx and with zvbi at Presentation
# Level 2.5 or 3.5 and compares them cell by cell. Needs a C compiler and
# zvbi's development files (pkg-config zvbi-0.2), so it runs on the Linux
# test host.
#   run_cells.sh NEOTTX LEVEL [--known FILE] FILE...
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
neottx="$1"
level="$2"
shift 2
skip=()
if [ "${1:-}" = "--known" ]; then
    skip=(--known "$2")
    shift 2
fi
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cc -O2 -Wall -Wextra -Werror "$here/zvbi_cells.c" $(pkg-config --cflags --libs zvbi-0.2) -o "$work/zvbi_cells"
status=0
for file in "$@"; do
    printf '\n== %s (Level %s)\n' "$(basename "$file")" "$level"
    python3 -I "$here/separate_subpages.py" "$file" "$work/input.t42"
    "$neottx" text "$work/input.t42" all --cells --level "$level" > "$work/neottx.txt" 2>/dev/null || true
    "$work/zvbi_cells" "$work/input.t42" "$level" > "$work/zvbi.txt"
    python3 -I "$here/compare_cells.py" "$work/neottx.txt" "$work/zvbi.txt" "${skip[@]}" || status=1
done
exit "$status"
