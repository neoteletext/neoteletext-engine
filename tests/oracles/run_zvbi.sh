#!/usr/bin/env bash
# REQ-TST-021: decodes T42 files with neottx and with the zvbi oracle and
# compares the results. Needs a C compiler and zvbi's development files
# (pkg-config zvbi-0.2), so it runs on the Linux test host.
#   run_zvbi.sh NEOTTX FILE[:GROUP]...
# FILE is T42, or a transport stream that neottx converts to T42 first. GROUP
# is the Table 32 character set group given to both decoders. neottx reveals
# concealed text, because zvbi prints it.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
neottx="$1"
shift
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cc -O2 -Wall -Wextra -Werror "$here/zvbi_pages.c" $(pkg-config --cflags --libs zvbi-0.2) -o "$work/zvbi_pages"
for item in "$@"; do
    file="${item%%:*}"
    group=0
    [ "$item" != "$file" ] && group="${item##*:}"
    printf '\n== %s (group %s)\n' "$(basename "$file")" "$group"
    case "$file" in
        *.t42) ;;
        *) "$neottx" convert "$file" --to t42 -o "$work/input.t42" 2>/dev/null; file="$work/input.t42" ;;
    esac
    "$neottx" text "$file" all --region "$group" --reveal > "$work/neottx.txt" 2>/dev/null || true
    "$work/zvbi_pages" "$file" "$group" > "$work/zvbi.txt"
    python3 -I "$here/compare_zvbi.py" "$work/neottx.txt" "$work/zvbi.txt"
done
