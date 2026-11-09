#!/bin/sh
# Claude Code PostToolUse hook. stdout stays empty; exit 2 returns feedback.
set -u

payload=$(cat)
if command -v jq >/dev/null 2>&1; then
    file=$(printf '%s' "$payload" | jq -er '.tool_input | (.file_path // .path // "") | strings') || {
        # Missing paths are normal for tools that did not edit a file.
        exit 0
    }
elif command -v python3 >/dev/null 2>&1; then
    file=$(printf '%s' "$payload" | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin).get("tool_input", {})
    path = data.get("file_path", data.get("path", ""))
    if isinstance(path, str):
        sys.stdout.write(path)
except (ValueError, AttributeError):
    sys.exit(1)
') || exit 0
else
    printf '%s\n' 'gdguard hook: install jq or Python 3 to read hook JSON.' >&2
    exit 2
fi

case "$file" in
    *.gd) ;;
    *) exit 0 ;;
esac
# Keep relative paths beginning with a dash from becoming CLI flags.
case "$file" in
    -*) file=./$file ;;
esac

binary=${GDGUARD_BIN:-gdguard}
if [ -z "${GDGUARD_BIN:-}" ] && ! command -v "$binary" >/dev/null 2>&1; then
    binary=gdscript-formatter
fi
report=$(mktemp) || exit 2
errors=$(mktemp) || { rm -f "$report"; exit 2; }
trap 'rm -f "$report" "$errors"' 0
trap 'exit 2' HUP INT TERM

"$binary" lint --fix --format json "$file" >"$report" 2>"$errors"
status=$?
case "$status" in
    0) exit 0 ;;
    1)
        printf '%s\n' 'gdguard: issues remain after safe fixes. Read help/fixes, fix the code, and lint again.' >&2
        cat "$report" >&2
        ;;
    *)
        printf 'gdguard: lint failed (exit %s). Fix the tool/config error before continuing.\n' "$status" >&2
        cat "$errors" >&2
        cat "$report" >&2
        ;;
esac
exit 2
