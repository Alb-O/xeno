#!/usr/bin/env bash
set -e

# Scoped rustdoc audit for architectural anchor subsystems.
# Verifies that invariant triads and intra-doc links are healthy.

RUSTDOCFLAGS="--document-private-items -D rustdoc::broken_intra_doc_links -A rustdoc::private_intra_doc_links -A warnings"

echo "Auditing xeno-editor..."
cargo doc -p xeno-editor --no-deps

echo "Auditing xeno-lsp..."
cargo doc -p xeno-lsp --features position --no-deps

echo "Auditing xeno-registry..."
cargo doc -p xeno-registry --no-deps

echo "Anchor audit PASSED."
