# Task 09B: Options System Integration & Cleanup

## Model Directive

This document specifies the completion of the options system by integrating config loading, replacing hardcoded values with option lookups, and removing dead code. Task 09A built the infrastructure; this task makes it functional.

**Context**: The options system has full infrastructure (derive macro, storage, resolver, commands, hooks) but:
- User config files are never loaded
- Options are never read in runtime code (hardcoded values)
- Dead code exists (unused static, inconsistent docs)

**Scope**: Wire up config loading at startup, replace hardcoded values with option access, remove dead code, fix documentation.

---

## Implementation Expectations

<mandatory_execution_requirements>

This is an **integration and cleanup** task. When implementing:

1. Make changes incrementally with verification after each phase
2. Run `cargo build --workspace` after structural changes
3. Run `cargo test --workspace` after each phase completion
4. Ensure no regressions in existing functionality

Unacceptable:
- Breaking the build between commits
- Leaving dead code after cleanup phases
- Introducing new hardcoded values

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Edit existing files; avoid creating new modules unless necessary
- Follow existing patterns for option access (see `OptionAccess` trait)
- Maintain backward compatibility for `:set` and `:setlocal` commands
- Do not change option semantics (defaults, types, scopes)

</verbosity_and_scope_constraints>

<design_freedom>

- Config loading location can be in `main.rs` or factored to a helper
- Option access can use either `buffer.option(key, editor)` or `editor.option_raw(key)`
- Dead code removal order is flexible

</design_freedom>

---

## Current State Analysis

### What Works
- `#[derive_option]` macro generates `TypedOptionKey<T>` constants
- `OptionStore` holds values, `OptionResolver` chains layers
- `:set` / `:setlocal` commands modify stores and emit hooks
- `Config::parse()` correctly parses KDL files

### What's Broken

| Issue | Location | Impact |
|-------|----------|--------|
| Config never loaded | `crates/term/src/main.rs` | User settings ignored |
| Options never used | `crates/api/src/render/`, `buffer/navigation.rs` | Hardcoded `tab_width = 4` |
| Dead static | `crates/registry/options/src/lib.rs:49` | `GLOBAL_OPTIONS` never used |
| Doc inconsistency | `crates/config/src/lib.rs:60` | `scrolloff` doesn't exist |
| No scope validation | `crates/api/src/capabilities.rs` | `:setlocal` accepts global options |

---

## Implementation Roadmap

### Phase 1: Load Config at Startup

**Objective**: Load `~/.config/xeno/config.kdl` and populate `Editor` option stores.

**Files**:
- `crates/term/src/main.rs`
- `crates/api/src/paths.rs` (already has `get_config_dir()`)

- [x] 1.1 Add config loading to `main.rs` after theme loading
  ```rust
  // After line 44 (theme loading)
  if let Some(config_dir) = xeno_api::paths::get_config_dir() {
      let config_path = config_dir.join("config.kdl");
      if config_path.exists() {
          match xeno_config::Config::load(&config_path) {
              Ok(config) => {
                  // Will apply to editor after creation
              }
              Err(e) => eprintln!("Warning: failed to load config: {}", e),
          }
      }
  }
  ```

- [x] 1.2 Apply parsed config to `Editor` after creation
  - Set `editor.global_options = config.options`
  - Populate `editor.language_options` from `config.languages`
  - Apply theme from config if specified (check `config.options` for theme key)

- [x] 1.3 Handle config-specified theme
  - If config contains `theme` option, call `editor.set_theme()` with that value
  - CLI `--theme` flag should override config (already happens after)

- [x] 1.4 Verify: Create test config file, run editor, confirm options loaded
  - Test with `:set tab-width` to see if config value is active

**Verification**: `cargo build -p xeno-term && cargo test -p xeno-term`

---

### Phase 2: Replace Hardcoded Option Values

**Objective**: Replace all hardcoded option values with actual option lookups.

**Files** (identified in review):
- `crates/api/src/buffer/navigation.rs:268` - `let tab_width = 4usize;`
- `crates/api/src/render/buffer/context.rs:227` - `let tab_width = 4usize;`
- `crates/api/src/render/types.rs:31` - `let tab_width = 4usize;`
- `crates/api/src/render/document/wrapping.rs:29` - `let tab_width = 4usize;`

- [x] 2.1 Update `navigation.rs` to use option
  - Function `screen_to_doc_position` needs `&Editor` or option value passed in
  - Pattern: Add parameter or access via existing editor reference
  - Replace: `let tab_width = buffer.option(keys::TAB_WIDTH, editor) as usize;`

- [x] 2.2 Update `render/buffer/context.rs` to use option
  - `BufferRenderContext` likely has editor access
  - Replace hardcoded value with option lookup

- [x] 2.3 Update `render/types.rs` to use option
  - May need to thread option value through or add editor reference

- [x] 2.4 Update `render/document/wrapping.rs` to use option
  - Wrapping calculations need tab width from options

- [x] 2.5 Search for any remaining hardcoded option values
  ```bash
  rg 'let tab_width = \d|let indent_width = \d|let scroll_margin = \d' crates/
  ```
  - Fix any additional occurrences found

- [x] 2.6 Verify: `cargo build --workspace` passes

**Threading Strategy**: If a function lacks editor access:
1. First choice: Add `&Editor` parameter if caller has it
2. Second choice: Add specific option value parameter (e.g., `tab_width: usize`)
3. Last resort: Use global default (only for truly global options)

---

### Phase 3: Remove Dead Code

**Objective**: Clean up unused infrastructure from failed/incomplete implementations.

**Files**:
- `crates/registry/options/src/lib.rs`

- [x] 3.1 Remove `GLOBAL_OPTIONS` static
  - Delete line 49: `static GLOBAL_OPTIONS: OnceLock<OptionStore> = OnceLock::new();`
  - Delete `init_global()` function (lines 431-433)
  - Delete `global()` function (lines 451-456)
  - Remove `OnceLock` import if unused

- [x] 3.2 Update any references to removed functions
  - Search: `rg 'init_global|GLOBAL_OPTIONS|options::global\(' crates/`
  - Should find only doc comments; update or remove them

- [x] 3.3 Clean up doc examples that reference removed code
  - Lines 31, 447-449 reference `global()` function
  - Update to show `Editor::global_options` pattern instead

- [x] 3.4 Verify: `cargo build --workspace` passes with no dead code warnings

**CLEANUP CHECKPOINT 1**: All dead option infrastructure removed

---

### Phase 4: Fix Documentation

**Objective**: Ensure documentation matches actual implementation.

**Files**:
- `crates/config/src/lib.rs` - module docs
- `crates/registry/options/src/lib.rs` - module docs
- Option definition files in `impls/`

- [x] 4.1 Fix `config/src/lib.rs` doc example
  - Line 60: `scrolloff 5` should be `scroll-margin 5`
  - Verify all option names in examples are valid KDL keys

- [x] 4.2 Update `options/src/lib.rs` module docs
  - Remove references to `global()` function
  - Add example showing config loading pattern
  - Add example showing `buffer.option(key, editor)` usage

- [x] 4.3 Verify option KDL keys match documentation
  - Cross-reference `impls/*.rs` option definitions with doc examples
  - Ensure all documented options actually exist

- [x] 4.4 Run doc tests
  - `cargo test --doc -p xeno-registry-options`
  - `cargo test --doc -p xeno-config`

---

### Phase 5: Add Scope Validation (Optional Enhancement)

**Objective**: Prevent setting global-scoped options with `:setlocal`.

**Files**:
- `crates/api/src/capabilities.rs`
- `crates/registry/options/src/lib.rs`

- [x] 5.1 Add scope check to `set_local_option`
  ```rust
  fn set_local_option(&mut self, kdl_key: &str, value: &str) -> Result<(), CommandError> {
      let def = find_by_kdl(kdl_key)
          .ok_or_else(|| CommandError::InvalidArgument(...))?;
      
      if def.scope == OptionScope::Global {
          return Err(CommandError::InvalidArgument(
              format!("'{}' is a global option, use :set instead", kdl_key)
          ));
      }
      // ... rest of implementation
  }
  ```

- [x] 5.2 Add test for scope validation
  - Test that `:setlocal theme gruvbox` returns error
  - Test that `:setlocal tab-width 2` succeeds

- [x] 5.3 Verify: `cargo test -p xeno-api`

---

### Phase 6: Integration Testing

**Objective**: Verify the complete options flow works end-to-end.

- [x] 6.1 Create integration test for config loading
  - Create temp config file with options
  - Load config, verify options in store
  - Test language-specific overrides

- [x] 6.2 Test option resolution hierarchy
  - Set global option via config
  - Override with `:setlocal`
  - Verify buffer-local wins

- [x] 6.3 Test hook emission
  - Set option via `:set`
  - Verify `OptionChanged` hook fires

- [x] 6.4 Final verification
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `cargo clippy --workspace`

---

## Architecture Reference

### Option Resolution Flow (Target State)

```
User config.kdl
    ↓
Config::load() at startup
    ↓
editor.global_options ← config.options
editor.language_options ← config.languages
    ↓
Runtime: buffer.option(keys::TAB_WIDTH, editor)
    ↓
OptionResolver::new()
    .with_buffer(&buffer.local_options)      ← :setlocal overrides
    .with_language(&editor.language_options) ← language "rust" { }
    .with_global(&editor.global_options)     ← options { } / :set
    .resolve(key)
    ↓
→ First match or compile-time default
```

### Key Types

```
TypedOptionKey<T>  ──→  OptionKey  ──→  &'static OptionDef
     │                                        │
     └── Compile-time type safety             └── Runtime metadata
                                                  (kdl_key, default, scope)
```

### Config File Location

```
$XDG_CONFIG_HOME/xeno/config.kdl
  └── Usually ~/.config/xeno/config.kdl on Linux
```

---

## Anti-Patterns

1. **Passing editor everywhere**: Don't thread `&Editor` through 10 layers just for one option. Extract the option value at the boundary and pass the primitive.

2. **Global mutable state**: Don't use `GLOBAL_OPTIONS` static. Use `Editor::global_options` instance field.

3. **String-based access in hot paths**: Use `TypedOptionKey<T>` for all option access. String lookup only at config parsing boundary.

4. **Ignoring scope**: Global options shouldn't be settable per-buffer. Validate scope in `:setlocal`.

---

## Success Criteria

- [x] Config file loaded at startup (when present)
- [x] All hardcoded option values replaced with lookups
- [x] `GLOBAL_OPTIONS` static and related functions removed
- [x] Documentation matches implementation
- [x] Scope validation prevents invalid `:setlocal` usage
- [x] All tests passing
- [x] No clippy warnings

---

## Files Summary

| File | Changes |
|------|---------|
| `crates/term/src/main.rs` | Add config loading |
| `crates/api/src/buffer/navigation.rs` | Use tab_width option |
| `crates/api/src/render/buffer/context.rs` | Use tab_width option |
| `crates/api/src/render/types.rs` | Use tab_width option |
| `crates/api/src/render/document/wrapping.rs` | Use tab_width option |
| `crates/registry/options/src/lib.rs` | Remove dead code, fix docs |
| `crates/api/src/capabilities.rs` | Add scope validation |
| `crates/config/src/lib.rs` | Fix doc examples |
