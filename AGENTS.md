# XENO

## Project Overview

Xeno is a TUI modal text editor written in Rust. Tree-sitter for syntax analysis, LSP IDE features. The architecture uses an registry pattern via the `inventory` crate, allowing components (actions, commands, motions, text objects) to register themselves at compile time without centralized wiring.

## Build and Test

The project uses Nix flakes with `direnv` or `nix develop -c` directly.

## Architecture

The workspace contains many crates under `crates/`. The main binary lives in `crates/term` and produces the `xeno` executable.

The `crates/registry/` subtree contains many sub-crates for components. [Read this](docs/agents/registry.md) if needing more context.

`xeno-lsp` implements the LSP client stack. [Read this](docs/agents/lsp.md) if needing more context.

`xeno-tui` is a modified Ratatui vendor. It renders to crossterm.

KDL files parsed by `xeno-runtime-config`. Runtime assets (queries, themes, language configs) inside `crates/runtime/data/assets` embed via `xeno-runtime-data`.
