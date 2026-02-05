set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

powershell := "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"

default:
    @just --list

# ================================================
# Windows host builds (WSL → native Windows cargo)
# ================================================

# Get Windows %TEMP% as forward slashes + WSL path
# Prints: "<WIN_TEMP_FWD_SLASH> <WSL_TEMP_DIR>"
_win-temp-pairs:
    @WP="$({{powershell}} -NoLogo -NoProfile -Command '[IO.Path]::GetTempPath()' | tr -d '\r\n')"; \
    WP="${WP//\\//}"; \
    WP="${WP%/}"; \
    WPWSL="$(wslpath -u "$WP")"; \
    printf '%s %s\n' "$WP" "$WPWSL"

# Get stable mirror location
# Prints: "<MIR_WP> <MIR_WS>"
_win-paths:
    @read WP WPWSL < <(just _win-temp-pairs); \
    MIR_WP="$WP/xeno"; \
    MIR_WS="$WPWSL/xeno"; \
    printf '%s %s\n' "$MIR_WP" "$MIR_WS"

# Sync workspace to Windows filesystem (git-tracked + untracked non-ignored files)
win-sync:
    read MIR_WP MIR_WS < <(just _win-paths); \
    mkdir -p "$MIR_WS"; \
    LOCK="/tmp/xeno-win-mirror.lock"; \
    exec 9>"$LOCK"; \
    flock 9; \
    if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then \
      TMPF="$(mktemp)"; \
      { git ls-files -z --recurse-submodules; git ls-files -z --others --exclude-standard; } > "$TMPF"; \
      rsync -a --delete --from0 --files-from="$TMPF" "$PWD"/ "$MIR_WS"/; \
      rm -f "$TMPF"; \
    else \
      rsync -a --delete --exclude ".git" --exclude "target" "$PWD"/ "$MIR_WS"/; \
    fi; \
    echo "Mirror synced → $MIR_WP"

# Write and run a PowerShell build/run script on Windows
# cargo_args: additional cargo arguments
_win-cargo +cargo_args:
    read MIR_WP MIR_WS < <(just _win-paths); \
    read WP WPWSL < <(just _win-temp-pairs); \
    PS_WS="$WPWSL/xeno-cargo.ps1"; \
    PS_WP="$WP/xeno-cargo.ps1"; \
    printf "%s\n" \
      "\$ErrorActionPreference='Stop'" \
      "Set-Location -LiteralPath '$MIR_WP'" \
      "cargo {{cargo_args}}" \
      > "$PS_WS"; \
    {{powershell}} -NoLogo -NoProfile -ExecutionPolicy Bypass -File "$PS_WP"

# Check workspace on Windows
win-check:
    just win-sync
    just _win-cargo check --workspace --all-targets

# Build xeno on Windows (debug)
win-build:
    just win-sync
    just _win-cargo build -p xeno-term

# Build xeno on Windows (release)
win-build-release:
    just win-sync
    just _win-cargo build --release -p xeno-term

# Sync, build, and run xeno on Windows (debug)
win-run +args='':
    just win-sync
    just _win-cargo run -p xeno-term -- {{args}}

# Sync, build, and run xeno on Windows (release)
win-run-release +args='':
    just win-sync
    just _win-cargo run --release -p xeno-term -- {{args}}

# Run workspace tests on Windows
win-test:
    just win-sync
    just _win-cargo test --workspace

# Hard reset: remove mirror, resync
win-reinit:
    read MIR_WP MIR_WS < <(just _win-paths); \
    rm -rf "$MIR_WS"; \
    {{powershell}} -NoLogo -NoProfile -Command \
      "\$p='$MIR_WP'; if(Test-Path \$p){ Remove-Item \$p -Recurse -Force }"; \
    just win-sync

# Remove Windows mirror entirely
win-clean:
    read MIR_WP MIR_WS < <(just _win-paths); \
    rm -rf "$MIR_WS"; \
    {{powershell}} -NoLogo -NoProfile -Command \
      "\$p='$MIR_WP'; if(Test-Path \$p){ Remove-Item \$p -Recurse -Force }"; \
    echo "Removed mirror."
