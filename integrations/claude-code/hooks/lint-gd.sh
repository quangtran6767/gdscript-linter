#!/bin/sh
# gdguard PostToolUse hook for Claude Code (Edit|Write|MultiEdit).
#
# Reads the hook JSON from stdin. When the edited file is a .gd file, it runs
#   gdguard lint --fix --format json <file>
# which applies safe fixes in place. Then:
#   - the file is clean (exit code 0): prints nothing and exits 0;
#   - issues remain: prints a short summary and the JSON report on stderr and
#     exits 2, so Claude sees the issues and fixes them;
#   - gdguard reports a config or rule-file error: prints it and exits 2.
# Other files, a missing file or a missing gdguard binary exit 0 (the hook
# never blocks work it can't check).
#
# Environment:
#   GDGUARD                 path of the gdguard binary (default: gdguard)
#   GDGUARD_HOOK_NO_JQ=1    ignore jq even when it is installed (for tests)
#
# POSIX sh. Uses jq when available; without jq it falls back to sed.

gdguard_bin=${GDGUARD:-gdguard}

input=$(cat)

has_jq=0
if [ "${GDGUARD_HOOK_NO_JQ:-0}" != "1" ] && command -v jq >/dev/null 2>&1; then
	has_jq=1
fi

# The edited file: .tool_input.file_path (Edit, Write and MultiEdit all use it).
if [ "$has_jq" = "1" ]; then
	file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')
else
	# First "file_path": "..." value; JSON escapes \\ and \" are undone.
	file_path=$(printf '%s' "$input" | tr -d '\n' \
		| sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\(\([^"\\]\|\\.\)*\)".*/\1/p' \
		| sed 's/\\"/"/g; s/\\\\/\\/g')
fi

case "$file_path" in
	*.gd) ;;
	*) exit 0 ;;
esac

if [ ! -f "$file_path" ]; then
	exit 0
fi

if ! command -v "$gdguard_bin" >/dev/null 2>&1 && [ ! -x "$gdguard_bin" ]; then
	echo "gdguard hook: '$gdguard_bin' not found; set GDGUARD or add gdguard to PATH. Skipping lint." >&2
	exit 0
fi

error_file=$(mktemp 2>/dev/null || echo "${TMPDIR:-/tmp}/gdguard-hook.$$")
report=$("$gdguard_bin" lint --fix --format json "$file_path" 2>"$error_file")
status=$?
# stderr holds "Fixed N problems in M files" and any errors; drop the line
# when nothing was fixed.
errors_text=$(grep -v '^Fixed 0 problems' "$error_file" 2>/dev/null)
rm -f "$error_file"

if [ "$status" = "0" ]; then
	exit 0
fi

if [ "$status" != "1" ]; then
	{
		echo "gdguard couldn't lint $file_path (exit code $status):"
		printf '%s\n' "$errors_text"
	} >&2
	exit 2
fi

# Issues remain after the safe fixes.
if [ "$has_jq" = "1" ]; then
	summary=$(printf '%s' "$report" | jq -r '.summary | "\(.errors) errors, \(.warnings) warnings, \(.infos) infos"')
	issue_lines=$(printf '%s' "$report" | jq -r '.issues[] | "  \(.file):\(.range.start.line):\(.range.start.column): \(.severity) [\(.rule)] \(.message)"')
else
	count_of() {
		printf '%s\n' "$report" | sed -n "s/.*\"$1\":[[:space:]]*\([0-9][0-9]*\).*/\1/p" | head -n 1
	}
	summary="$(count_of errors) errors, $(count_of warnings) warnings, $(count_of infos) infos"
	issue_lines=""
fi

{
	echo "gdguard: $file_path still has issues after safe fixes ($summary)."
	if [ -n "$issue_lines" ]; then
		printf '%s\n' "$issue_lines"
	fi
	if [ -n "$errors_text" ]; then
		printf '%s\n' "$errors_text"
	fi
	echo "Fix them using each issue's \"help\" and \"fixes\" (run \`gdguard explain <rule>\` when unsure), then the hook lints again. JSON report:"
	printf '%s\n' "$report"
} >&2
exit 2
