#!/usr/bin/env bash
# Sync tree-sitter queries from Helix (queries only, not languages.toml).
# Usage: ./scripts/sync-helix-runtime.sh [ref]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HELIX_REPO="https://github.com/helix-editor/helix.git"
REF="${1:-master}"

# Temporary directory for sparse checkout
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

echo "Syncing Helix runtime data (ref: $REF)..."

# Clone with sparse checkout
cd "$WORK_DIR"
git init -q
git remote add origin "$HELIX_REPO"
git config core.sparseCheckout true

# Only fetch what we need (queries only, not languages.toml)
cat > .git/info/sparse-checkout << 'EOF'
/runtime/queries/
EOF

echo "Fetching from helix-editor/helix..."
git fetch --depth=1 origin "$REF" -q
git checkout FETCH_HEAD -q

# Get the commit hash for provenance
COMMIT_HASH=$(git rev-parse HEAD)
COMMIT_DATE=$(git log -1 --format=%ci)

echo "Synced from commit: $COMMIT_HASH"
echo "Commit date: $COMMIT_DATE"

# Sync queries only (languages.toml is maintained separately)
echo "Copying runtime/queries/..."
rm -rf "$REPO_ROOT/runtime/queries"
cp -r runtime/queries "$REPO_ROOT/runtime/"

# Write provenance file
cat > "$REPO_ROOT/runtime/.helix-sync" << EOF
upstream = "https://github.com/helix-editor/helix"
ref = "$REF"
commit = "$COMMIT_HASH"
synced_at = "$(date -Iseconds)"
EOF

# Count what we synced
LANG_COUNT=$(find "$REPO_ROOT/runtime/queries" -mindepth 1 -maxdepth 1 -type d | wc -l)
SCM_COUNT=$(find "$REPO_ROOT/runtime/queries" -name "*.scm" | wc -l)

echo ""
echo "Sync complete!"
echo "  Languages: $LANG_COUNT"
echo "  Query files: $SCM_COUNT"
echo "  Provenance: runtime/.helix-sync"
