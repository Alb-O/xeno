#!/usr/bin/env bash
set -euo pipefail

nix develop -c cargo run -p xeno-registry --bin registry_inventory | sed '/^imp-lint:/d' > docs/registry-inventory.md
