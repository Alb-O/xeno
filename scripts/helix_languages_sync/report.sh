#!/usr/bin/env bash
# Generate a review report for upstream Helix languages.toml changes.
#
# Usage:
#   ./scripts/helix_languages_sync/report.sh [base_ref] [head_ref] [output_md]
#
# Defaults:
# * base_ref from DEFAULT_BASE_REF below (or HELIX_LANG_SYNC_BASE_REF env)
# * head_ref from DEFAULT_HEAD_REF below (or HELIX_LANG_SYNC_HEAD_REF env)
# * output_md: tmp/helix_languages_sync_report_<base8>_<head8>.md
#
# Side effects:
# * Maintains a sparse Helix checkout at target/external/helix-languages
# * Writes report markdown and a sibling .patch file

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
CACHE_REPO="$REPO_ROOT/target/external/helix-languages"
TRACKED_FILE="languages.toml"

# Sync baseline metadata is intentionally in-script (single-file workflow).
HELIX_UPSTREAM="https://github.com/helix-editor/helix"
DEFAULT_HEAD_REF="master"
DEFAULT_BASE_REF="1a38979aaa53ea96425a04413c871600ee5845e7"

if ! command -v git >/dev/null 2>&1; then
	echo "error: git is required" >&2
	exit 1
fi

if ! command -v rg >/dev/null 2>&1; then
	echo "error: rg is required" >&2
	exit 1
fi

if [[ -z "$HELIX_UPSTREAM" || -z "$DEFAULT_HEAD_REF" || -z "$DEFAULT_BASE_REF" ]]; then
	echo "error: script defaults are not configured" >&2
	exit 1
fi

BASE_REF="${1:-${HELIX_LANG_SYNC_BASE_REF:-$DEFAULT_BASE_REF}}"
HEAD_REF="${2:-${HELIX_LANG_SYNC_HEAD_REF:-$DEFAULT_HEAD_REF}}"
OUTPUT_MD="${3:-}"

if [[ -z "$BASE_REF" ]]; then
	echo "error: no base ref provided and DEFAULT_BASE_REF is empty" >&2
	echo "hint: pass an explicit base ref: ./scripts/helix_languages_sync/report.sh <base_ref>" >&2
	exit 1
fi

mkdir -p "$CACHE_REPO"

if [[ ! -d "$CACHE_REPO/.git" ]]; then
	git -C "$CACHE_REPO" init -q
	git -C "$CACHE_REPO" remote add origin "$HELIX_UPSTREAM"
	git -C "$CACHE_REPO" config core.sparseCheckout true
	mkdir -p "$CACHE_REPO/.git/info"
	printf '/%s\n' "$TRACKED_FILE" > "$CACHE_REPO/.git/info/sparse-checkout"
fi

git -C "$CACHE_REPO" fetch --depth=400 origin "$HEAD_REF" -q
HEAD_COMMIT="$(git -C "$CACHE_REPO" rev-parse FETCH_HEAD)"
git -C "$CACHE_REPO" checkout -q "$HEAD_COMMIT"

if ! git -C "$CACHE_REPO" cat-file -e "$BASE_REF^{commit}" 2>/dev/null; then
	git -C "$CACHE_REPO" fetch origin "$BASE_REF" -q || true
fi
if ! git -C "$CACHE_REPO" cat-file -e "$BASE_REF^{commit}" 2>/dev/null; then
	echo "error: base ref '$BASE_REF' not found in Helix repo" >&2
	exit 1
fi

if [[ -z "$OUTPUT_MD" ]]; then
	mkdir -p "$REPO_ROOT/tmp"
	OUTPUT_MD="$REPO_ROOT/tmp/helix_languages_sync_report_${BASE_REF:0:8}_${HEAD_COMMIT:0:8}.md"
fi
OUTPUT_PATCH="${OUTPUT_MD%.md}.patch"

BASE_DATE="$(git -C "$CACHE_REPO" log -1 --date=iso-strict --pretty='%cd' "$BASE_REF")"
HEAD_DATE="$(git -C "$CACHE_REPO" log -1 --date=iso-strict --pretty='%cd' "$HEAD_COMMIT")"

COMMIT_LINES="$(git -C "$CACHE_REPO" log --reverse --date=short --pretty='* %h %ad %s' "$BASE_REF..$HEAD_COMMIT" -- "$TRACKED_FILE" || true)"
DIFFSTAT="$(git -C "$CACHE_REPO" diff --stat "$BASE_REF" "$HEAD_COMMIT" -- "$TRACKED_FILE" || true)"

git -C "$CACHE_REPO" diff "$BASE_REF" "$HEAD_COMMIT" -- "$TRACKED_FILE" > "$OUTPUT_PATCH"

HINTS="$(git -C "$CACHE_REPO" diff -U0 "$BASE_REF" "$HEAD_COMMIT" -- "$TRACKED_FILE" \
	| rg '^[+-](\[\[language\]\]|\[\[language-server\]\]|name = |grammar = |file-types = |roots = |language-servers = |auto-format = |formatter = |[A-Za-z0-9_.-]+ = \{ command = )' \
	|| true)"

{
	echo "# Helix languages.toml sync report"
	echo
	echo "* upstream: $HELIX_UPSTREAM"
	echo "* tracked file: $TRACKED_FILE"
	echo "* base: $BASE_REF ($BASE_DATE)"
	echo "* head: $HEAD_COMMIT ($HEAD_DATE)"
	echo "* generated_at: $(date -Iseconds)"
	echo
	echo "## Commits touching languages.toml"
	if [[ -n "$COMMIT_LINES" ]]; then
		echo "$COMMIT_LINES"
	else
		echo "* no changes touching languages.toml in this range"
	fi
	echo
	echo "## Diffstat"
	if [[ -n "$DIFFSTAT" ]]; then
		echo '```text'
		echo "$DIFFSTAT"
		echo '```'
	else
		echo "_no diff_"
	fi
	echo
	echo "## Entry-level change hints"
	echo "_Use this to locate impacted language/server blocks quickly._"
	if [[ -n "$HINTS" ]]; then
		echo '```diff'
		echo "$HINTS"
		echo '```'
	else
		echo "_no matching language/server lines changed_"
	fi
	echo
	echo "## Full patch"
	echo "\`$OUTPUT_PATCH\`"
	echo
	echo "## Suggested application flow"
	echo "1. Review each commit in order and classify change type:"
	echo "   * language metadata -> \`crates/registry/src/domains/languages/assets/languages.nuon\`"
	echo "   * LSP server definition -> \`crates/registry/src/domains/lsp_servers/assets/lsp_servers.nuon\`"
	echo "   * grammar source/revision -> \`crates/registry/src/domains/grammars/assets/grammars.nuon\`"
	echo "2. Translate only representable fields into Xeno schema; skip unsupported Helix-only knobs."
	echo "3. Validate:"
	echo "   * \`cargo check -p xeno-registry\`"
	echo "   * \`cargo test -p xeno-language test_queries_from_registry\`"
	echo "   * \`cargo check --workspace\`"
	echo "4. After merge, update DEFAULT_BASE_REF in \`scripts/helix_languages_sync/report.sh\` to the reviewed head commit."
} > "$OUTPUT_MD"

echo "Report written: $OUTPUT_MD"
echo "Patch written:  $OUTPUT_PATCH"
