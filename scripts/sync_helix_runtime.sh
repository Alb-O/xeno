#!/usr/bin/env bash
# Update the pinned Helix runtime commit used as external query dependency.
# Usage: ./scripts/sync_helix_runtime.sh [ref]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
HELIX_REPO="https://github.com/helix-editor/helix.git"
LOCK_FILE="$REPO_ROOT/crates/registry/src/domains/languages/assets/helix_runtime.nuon"
CACHE_ROOT="$REPO_ROOT/target/external/helix-runtime"
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

# Update lock metadata consumed by registry build.
cat > "$LOCK_FILE" << EOF
{
  upstream: "https://github.com/helix-editor/helix",
  ref: "$REF",
  commit: "$COMMIT_HASH",
  synced_at: "$(date -Iseconds)"
}
EOF

# Warm external cache used by build script to avoid an extra network fetch.
mkdir -p "$CACHE_ROOT"
rm -rf "$CACHE_ROOT/$COMMIT_HASH"
cp -a "$WORK_DIR" "$CACHE_ROOT/$COMMIT_HASH"

# Count what we synced
LANG_COUNT=$(find "$WORK_DIR/runtime/queries" -mindepth 1 -maxdepth 1 -type d | wc -l)
SCM_COUNT=$(find "$WORK_DIR/runtime/queries" -name "*.scm" | wc -l)

echo ""
echo "Sync complete!"
echo "  Languages: $LANG_COUNT"
echo "  Query files: $SCM_COUNT"
echo "  Lock file: crates/registry/src/domains/languages/assets/helix_runtime.nuon"
echo "  Warm cache: target/external/helix-runtime/$COMMIT_HASH"
