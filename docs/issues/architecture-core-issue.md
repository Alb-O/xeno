# Architecture core issue: current state summary

Date: 2026-02-12

## Purpose

This document tracks the architecture split between `xeno-editor` (core engine) and `xeno-editor-tui` (frontend/runtime), with a concise status view instead of per-commit logs.

## Target model

- `xeno-editor` owns editor state, document operations, policies, effects, and backend-neutral render data.
- `xeno-editor-tui` owns terminal runtime, frame composition, widget rendering, and frontend interaction wiring.
- Engine/frontend coordination happens through data APIs and explicit seams, not internal field reach-through.

## Current state

### Ownership

- Frontend runtime/composition is in `xeno-editor-tui`.
- Completion, snippet-choice, notifications, status line, and info-popup surface rendering are frontend-owned.
- Core keeps state/plan APIs for these systems and does not own terminal widget composition.

### Boundary hardening completed

- `xeno-editor` no longer has direct `xeno_tui` usage in `crates/editor/src` (current count: `0`).
- `xeno-editor` no longer publicly exposes broad internal modules:
  - `render` is internal; frontend imports via `render_api`.
  - `impls` is internal; focused symbols are re-exported at crate root where needed.
  - `notifications` internals are hidden behind typed editor APIs.
- Legacy compatibility paths/shims removed:
  - focus compatibility helper path removed in favor of unified `set_focus` flow.
  - unused overlay compatibility constructor argument removed.
  - stale render seam exports and dead helper surface removed.
- Umbrella passthrough patterns were reduced:
  - callsites now import types from owning modules (`types`, `command_queue`, etc.).

### Feature posture

- `xeno-editor` supports headless builds (`--no-default-features`).
- `tui` remains an optional feature on `xeno-editor` (default-enabled), with frontend runtime in `xeno-editor-tui`.

## Remaining work

- Decide whether panel registration/state should remain in core `UiManager` or move fully to frontend ownership.
- Continue tightening render data seams where frontend still depends on broader context than desired.
- Keep pruning legacy/compat patterns as they appear during refactors.

## Acceptance checks

Use this matrix for boundary-sensitive changes:

```bash
cargo check -p xeno-editor --all-targets
cargo check -p xeno-editor --no-default-features --all-targets
cargo check -p xeno-editor-tui
cargo check -p xeno-term
```

Run targeted tests for touched areas (`focus`, `mouse`, `row`, panel/overlay tests) before merge.

## History policy

Detailed checkpoint history is intentionally kept in Git commit history and PRs.

Useful resume commands:

```bash
git log --oneline -n 40
rg -n "xeno_tui::|use xeno_tui" crates/editor/src -S
rg -n "xeno_editor::render::|xeno_editor::impls::|xeno_editor::notifications::" crates/editor-tui/src crates/term/src -S
```
