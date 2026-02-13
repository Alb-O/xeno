# Render seam contract

Frontends consume core-owned, opaque plan types exclusively via `xeno_editor::render_api`.
Internal modules are `pub(crate)` — frontends cannot name internal paths.

## Plan APIs (core → frontends)

| API | Returns |
|---|---|
| `document_view_plans(bounds)` | `Vec<DocumentViewPlan>` |
| `separator_scene_plan(bounds)` | `SeparatorScenePlan` |
| `overlay_pane_view_plans()` | `Vec<OverlayPaneViewPlan>` |
| `overlay_completion_menu_target()` | `Option<OverlayCompletionMenuTarget>` |
| `info_popup_view_plans(bounds)` | `Vec<InfoPopupViewPlan>` |
| `completion_popup_render_plan()` | `Option<CompletionRenderPlan>` |
| `snippet_choice_render_plan()` | `Option<SnippetChoiceRenderPlan>` |
| `statusline_render_plan()` | `Vec<StatuslineRenderSegment>` |
| `statusline_rows()` | `u16` |

All returned types have private fields with getter-only access.

## Enforcement

- **Compile-time**: internal modules are `pub(crate)`, plan struct fields are `pub(crate)`.
- **Runtime test**: `seam_contract::tests::frontend_sources_use_only_render_api_seam` scans frontend source files for forbidden internal path patterns.
- **Grep proof**: `rg "xeno_editor::(completion|snippet|overlay|ui|info_popup|window|geometry|render)::" crates/editor-(tui|iced)` must be empty.

## How to add a new render feature

1. Define plan struct in a core module (e.g. `crates/editor/src/render/`).
2. Make fields `pub(crate)`, add getter methods.
3. Add `Editor::new_plan_api()` method.
4. Re-export the plan type via `crates/editor/src/render_api.rs`.
5. Update frontends to consume the new plan via getters.
6. Add the internal path to `seam_contract.rs` forbidden patterns.

## Non-goals

- Frontends must not assemble render contexts, touch caches, or make policy decisions.
- No `pub` fields on plan types — getters only.

## Hygiene commands

```bash
# seam boundary grep (must be empty in frontend crates)
rg -n "xeno_editor::(completion|snippet|overlay|ui|info_popup|window|geometry|render)::" crates/editor-tui/src crates/editor-iced/src

# full check matrix
cargo check --workspace --all-targets
cargo test -p xeno-editor
cargo test -p xeno-editor seam_contract
nix fmt
```
