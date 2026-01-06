# Task 09C: Options System Cleanup & Gap-Filling

## Model Directive

This document specifies the cleanup of the options system by removing unused options, centralizing validation, consolidating access patterns, and documenting what remains. Task 09B wired up config loading; this task addresses the significant gaps and redundancies identified in review.

**Context**: The options system has 22 defined options but only 2 are actually used (`TAB_WIDTH`, `THEME`). The rest are placeholder definitions with no implementation. Validation is scattered, access patterns are inconsistent, and scope validation only happens at runtime.

**Scope**: Remove dead options OR implement missing features, centralize validation, consolidate API surface, fix scope validation at parse time.

---

## Implementation Expectations

<mandatory_execution_requirements>

This is a **cleanup and consolidation** task. When implementing:

1. Make changes incrementally with verification after each phase
2. Run `cargo build --workspace` after structural changes
3. Run `cargo test --workspace` after each phase completion
4. Ensure no regressions in existing functionality
5. Update all affected tests when removing/modifying options

Unacceptable:
- Leaving half-implemented options in place
- Breaking existing `:set`/`:setlocal` functionality
- Introducing new scattered validation logic

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Remove unused options rather than implementing features for them (unless specifically requested)
- Keep the typed option access pattern (`buffer.option(key, editor)`)
- Maintain backward compatibility for config files (warn on removed options, don't error)
- Follow existing validation patterns but centralize them

</verbosity_and_scope_constraints>

<design_freedom>

- Validator function signature can be designed as needed
- Deprecation warning approach is flexible (log, stderr, notification)
- Order of cleanup phases can be adjusted based on dependencies

</design_freedom>

---

## Current State Analysis

### Options Usage Summary

| Option | Defined | Used | Status |
|--------|---------|------|--------|
| `TAB_WIDTH` | `impls/indent.rs:8` | `editor/views.rs:101,107` | **Active** |
| `THEME` | `impls/theme.rs:11` | `term/main.rs:85` | **Active** |
| `INDENT_WIDTH` | `impls/indent.rs:13` | Never | Dead |
| `USE_TABS` | `impls/indent.rs:18` | Never | Dead |
| `LINE_NUMBERS` | `impls/display.rs:8` | Never | Dead |
| `WRAP_LINES` | `impls/display.rs:13` | Never | Dead |
| `CURSORLINE` | `impls/display.rs:18` | Never | Dead |
| `CURSORCOLUMN` | `impls/display.rs:23` | Never | Dead |
| `COLORCOLUMN` | `impls/display.rs:28` | Never | Dead |
| `WHITESPACE_VISIBLE` | `impls/display.rs:33` | Never | Dead |
| `SCROLL_MARGIN` | `impls/scroll.rs:8` | Never | Dead |
| `SCROLL_SMOOTH` | `impls/scroll.rs:13` | Never | Dead |
| `BACKUP` | `impls/file.rs:8` | Never | Dead |
| `UNDO_FILE` | `impls/file.rs:13` | Never | Dead |
| `AUTO_SAVE` | `impls/file.rs:18` | Never | Dead |
| `FINAL_NEWLINE` | `impls/file.rs:23` | Never | Dead |
| `TRIM_TRAILING_WHITESPACE` | `impls/file.rs:28` | Never | Dead |
| `SEARCH_CASE_SENSITIVE` | `impls/search.rs:8` | Never | Dead |
| `SEARCH_SMART_CASE` | `impls/search.rs:13` | Never | Dead |
| `SEARCH_WRAP` | `impls/search.rs:18` | Never | Dead |
| `INCREMENTAL_SEARCH` | `impls/search.rs:23` | Never | Dead |
| `MOUSE` | `impls/behavior.rs:8` | Never | Dead |
| `LINE_ENDING` | `impls/behavior.rs:13` | Never | Dead |
| `IDLE_TIMEOUT` | `impls/behavior.rs:18` | Never | Dead |

### Key Issues

1. **Dead Options**: 20 of 22 options are defined but never read
2. **Scattered Validation**: `tab-width`/`indent-width` validated in `capabilities.rs:51-57`, not in option system
3. **Duplicate Resolution Logic**: Same code in `buffer/mod.rs:305-316` and `capabilities.rs:424-442`
4. **No Parse-Time Scope Validation**: Global options accepted in language blocks silently
5. **Multiple Access Patterns**: 4 ways to access options causes confusion

---

## Implementation Roadmap

### Phase 1: Remove Dead Options (Major Cleanup)

**Objective**: Remove all options that have no implementation, reducing noise and confusion.

**Files**:
- `crates/registry/options/src/impls/indent.rs`
- `crates/registry/options/src/impls/display.rs`
- `crates/registry/options/src/impls/scroll.rs`
- `crates/registry/options/src/impls/file.rs`
- `crates/registry/options/src/impls/search.rs`
- `crates/registry/options/src/impls/behavior.rs`

- [x] 1.1 Remove unused indent options
  - Keep: `TAB_WIDTH`
  - Remove: `INDENT_WIDTH`, `USE_TABS`
  - Update `impls/indent.rs` to only contain `TAB_WIDTH`

- [x] 1.2 Remove all display options (none are used)
  - Remove entire file `impls/display.rs`
  - Remove from `impls/mod.rs` module declaration

- [x] 1.3 Remove all scroll options (none are used)
  - Remove entire file `impls/scroll.rs`
  - Remove from `impls/mod.rs` module declaration

- [x] 1.4 Remove all file options (none are used)
  - Remove entire file `impls/file.rs`
  - Remove from `impls/mod.rs` module declaration

- [x] 1.5 Remove all search options (none are used)
  - Remove entire file `impls/search.rs`
  - Remove from `impls/mod.rs` module declaration

- [x] 1.6 Remove all behavior options (none are used)
  - Remove entire file `impls/behavior.rs`
  - Remove from `impls/mod.rs` module declaration

- [x] 1.7 Update `keys` module re-exports
  - File: `crates/registry/options/src/lib.rs:75-83`
  - Remove re-exports for deleted modules
  - Keep only: `indent::*`, `theme::*`

- [x] 1.8 Update tests that reference removed options
  - Search: `rg "keys::(INDENT_WIDTH|USE_TABS|SCROLL_MARGIN|CURSORLINE)" --type rust`
  - Update or remove affected test cases
  - Most are in `store.rs` and `resolver.rs` tests - update to use only `TAB_WIDTH`/`THEME`

- [x] 1.9 Verify build and tests pass
  - `cargo build --workspace`
  - `cargo test --workspace`

**CLEANUP CHECKPOINT 1**: Only `TAB_WIDTH` and `THEME` options remain

---

### Phase 2: Centralize Validation

**Objective**: Move validation logic from scattered locations into the option definition system.

**Files**:
- `crates/registry/options/src/lib.rs`
- `crates/macro/src/option.rs`
- `crates/api/src/capabilities.rs`

- [x] 2.1 Add optional validator field to `OptionDef`
  ```rust
  // In lib.rs, add to OptionDef struct
  pub struct OptionDef {
      // ... existing fields ...
      /// Optional validator for value constraints.
      /// Returns `Ok(())` if valid, `Err(reason)` if invalid.
      pub validator: Option<fn(&OptionValue) -> Result<(), String>>,
  }
  ```

- [x] 2.2 Update macro to include validator field
  - File: `crates/macro/src/option.rs`
  - Add `validator: None` to generated `OptionDef`
  - (Future: add `#[option(validate = "...")]` attribute support)

- [x] 2.3 Create standard validators module
  - File: `crates/registry/options/src/validators.rs`
  ```rust
  /// Validates that an integer is positive (>= 1).
  pub fn positive_int(value: &OptionValue) -> Result<(), String> {
      match value {
          OptionValue::Int(n) if *n >= 1 => Ok(()),
          OptionValue::Int(n) => Err(format!("must be at least 1, got {}", n)),
          _ => Err("expected integer".to_string()),
      }
  }
  ```

- [x] 2.4 Apply validator to `TAB_WIDTH`
  - File: `crates/registry/options/src/impls/indent.rs`
  - Manually set validator in the static (macro doesn't support it yet)
  - Or: update macro to accept `#[option(validate = "positive_int")]`

- [x] 2.5 Create central validation function
  - File: `crates/registry/options/src/lib.rs`
  ```rust
  /// Validates a value against an option's constraints.
  pub fn validate_value(kdl_key: &str, value: &OptionValue) -> Result<(), OptionError> {
      let def = find_by_kdl(kdl_key)
          .ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
      
      if !value.matches_type(def.value_type) {
          return Err(OptionError::TypeMismatch { ... });
      }
      
      if let Some(validator) = def.validator {
          validator(value).map_err(|reason| OptionError::InvalidValue {
              option: kdl_key.to_string(),
              reason,
          })?;
      }
      
      Ok(())
  }
  ```

- [x] 2.6 Remove hardcoded validation from capabilities.rs
  - File: `crates/api/src/capabilities.rs:51-57`
  - Replace with call to `validate_value()`
  - Delete the hardcoded `tab-width`/`indent-width` check

- [x] 2.7 Add validation to config parsing
  - File: `crates/config/src/options.rs`
  - Call `validate_value()` after type checking
  - Emit warning (not error) for invalid values in config files

- [x] 2.8 Verify: `cargo build --workspace && cargo test --workspace`

**CLEANUP CHECKPOINT 2**: All validation centralized in option system

---

### Phase 3: Consolidate Resolution Logic

**Objective**: Remove duplicate resolution code by extracting a shared helper.

**Files**:
- `crates/api/src/editor/mod.rs`
- `crates/api/src/buffer/mod.rs`
- `crates/api/src/capabilities.rs`

- [x] 3.1 Add `resolve_option` method to `Editor`
  - File: `crates/api/src/editor/mod.rs` or new `crates/api/src/editor/options.rs`
  ```rust
  impl Editor {
      /// Resolves an option for a specific buffer through the full hierarchy.
      pub fn resolve_option(&self, buffer_id: BufferId, key: OptionKey) -> OptionValue {
          let buffer = self.buffers.get_buffer(buffer_id)
              .expect("buffer must exist");
          
          let mut resolver = OptionResolver::new()
              .with_buffer(&buffer.local_options)
              .with_global(&self.global_options);
          
          if let Some(lang_store) = buffer.file_type()
              .and_then(|ft| self.language_options.get(&ft))
          {
              resolver = resolver.with_language(lang_store);
          }
          
          resolver.resolve(key)
      }
      
      /// Resolves a typed option for the focused buffer.
      pub fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T {
          let buffer_id = self.focused_view();
          T::from_option(&self.resolve_option(buffer_id, key.untyped()))
              .or_else(|| T::from_option(&(key.def().default)()))
              .expect("option type mismatch")
      }
  }
  ```

- [x] 3.2 Update `buffer.option_raw()` to delegate
  - File: `crates/api/src/buffer/mod.rs:305-316`
  - Change to call `editor.resolve_option(self.id, key)`
  - Or keep as-is if buffer doesn't have access to its own ID

- [x] 3.3 Update `OptionAccess` impl for Editor
  - File: `crates/api/src/capabilities.rs:423-442`
  - Delegate to `self.resolve_option()` instead of inline logic

- [x] 3.4 Verify no duplicate resolution code remains
  - Search: `rg "OptionResolver::new\(\)" crates/api/`
  - Should only appear in the new shared method

- [x] 3.5 Verify: `cargo build --workspace && cargo test --workspace`

**CLEANUP CHECKPOINT 3**: Single source of truth for option resolution

---

### Phase 4: Add Parse-Time Scope Validation

**Objective**: Warn when config files use options incorrectly (e.g., global option in language block).

**Files**:
- `crates/config/src/options.rs`
- `crates/config/src/lib.rs`

- [x] 4.1 Add scope context to option parsing
  - File: `crates/config/src/options.rs`
  ```rust
  #[derive(Clone, Copy)]
  pub enum ParseContext {
      Global,    // Inside `options { }`
      Language,  // Inside `language "foo" { }`
  }
  
  pub fn parse_options_with_context(
      node: &KdlNode,
      context: ParseContext,
  ) -> Result<(OptionStore, Vec<ConfigWarning>)> {
      let mut warnings = Vec::new();
      // ... existing parsing ...
      
      // Warn on scope mismatch
      if context == ParseContext::Language && def.scope == OptionScope::Global {
          warnings.push(ConfigWarning::ScopeMismatch {
              option: kdl_key.to_string(),
              found_in: "language block",
              expected: "global options block",
          });
      }
      
      Ok((store, warnings))
  }
  ```

- [x] 4.2 Define `ConfigWarning` type
  - File: `crates/config/src/error.rs`
  ```rust
  #[derive(Debug)]
  pub enum ConfigWarning {
      ScopeMismatch {
          option: String,
          found_in: &'static str,
          expected: &'static str,
      },
      UnknownOption {
          key: String,
          suggestion: Option<String>,
      },
  }
  ```

- [x] 4.3 Update `Config::parse()` to collect and return warnings
  - File: `crates/config/src/lib.rs`
  - Change return type to `Result<(Config, Vec<ConfigWarning>)>`
  - Or add `Config::warnings` field

- [x] 4.4 Display warnings at startup
  - File: `crates/term/src/main.rs`
  - After config load, print any warnings to stderr

- [x] 4.5 Add test for scope warning
  ```rust
  #[test]
  fn test_global_option_in_language_block_warns() {
      let kdl = r#"
  language "rust" {
      theme "gruvbox"
  }
  "#;
      let (config, warnings) = Config::parse(kdl).unwrap();
      assert!(!warnings.is_empty());
      assert!(warnings[0].to_string().contains("theme"));
  }
  ```

- [x] 4.6 Verify: `cargo build --workspace && cargo test --workspace`

---

### Phase 5: Documentation Update

**Objective**: Update all documentation to reflect the cleaned-up system.

**Files**:
- `crates/registry/options/src/lib.rs`
- `crates/config/src/lib.rs`
- Option impl files

- [x] 5.1 Update module docs in `options/src/lib.rs`
  - Remove references to removed options
  - Document only `TAB_WIDTH` and `THEME`
  - Add validator documentation
  - Update examples

- [x] 5.2 Update config crate docs
  - File: `crates/config/src/lib.rs`
  - Update example config to only show valid options
  - Document warning system

- [x] 5.3 Add deprecation note for removed options
  - Consider: Add `find_deprecated()` function that recognizes old option names
  - Provide helpful message: "scroll-margin was removed; this option is not yet implemented"

- [x] 5.4 Run doc tests
  - `cargo test --doc --workspace`

- [x] 5.5 Final verification
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `cargo clippy --workspace`

---

## Alternative: Implement Features Instead of Removing

If the goal is to **implement** the unused options rather than remove them, replace Phase 1 with:

### Phase 1-ALT: Implement Missing Option Features

- [ ] 1-ALT.1 Implement `CURSORLINE` option
  - File: `crates/api/src/render/buffer/context.rs`
  - Check option before applying `cursorline_bg`
  - When false, skip cursor line highlighting

- [ ] 1-ALT.2 Implement `SCROLL_MARGIN` option
  - File: `crates/api/src/buffer/navigation.rs`
  - Add `ensure_cursor_visible()` function
  - Respect margin when scrolling

- [ ] 1-ALT.3 Implement `WRAP_LINES` option
  - File: `crates/api/src/render/buffer/context.rs`
  - When false, render single line per doc line
  - Horizontal scroll instead of wrap

(Continue for each option...)

**Note**: This is significantly more work. The removal approach is recommended unless these features are actively requested.

---

## Architecture Reference

### Target Option System (After Cleanup)

```
┌────────────────────────────────────────────────────────────────┐
│ OPTION DEFINITIONS (2 total)                                    │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  impls/indent.rs:                                              │
│    TAB_WIDTH: i64 = 4  (buffer-scoped, validator: positive)    │
│                                                                 │
│  impls/theme.rs:                                               │
│    THEME: String = "gruvbox"  (global-scoped)                  │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│ VALIDATION                                                      │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  OptionDef.validator: Option<fn(&OptionValue) -> Result<()>>   │
│    └── Called by validate_value() in lib.rs                   │
│    └── Called from capabilities.rs (commands)                 │
│    └── Called from config/options.rs (parse)                  │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│ RESOLUTION (single path)                                        │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Editor::resolve_option(buffer_id, key)                        │
│    └── buffer.local_options                                   │
│    └── language_options[file_type]                            │
│    └── global_options                                         │
│    └── OptionDef.default                                       │
│                                                                 │
├────────────────────────────────────────────────────────────────┤
│ ACCESS PATTERNS (consolidated)                                  │
├────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Preferred:                                                    │
│    editor.option(keys::TAB_WIDTH)  →  i64                     │
│    buffer.option(keys::TAB_WIDTH, editor)  →  i64             │
│                                                                 │
│  Internal:                                                     │
│    editor.resolve_option(buffer_id, key)  →  OptionValue      │
│                                                                 │
└────────────────────────────────────────────────────────────────┘
```

### Config Warning Flow

```
config.kdl
    │
    ▼
Config::parse()
    │
    ├── Valid options → OptionStore
    │
    └── Scope mismatches → Vec<ConfigWarning>
                               │
                               ▼
                         stderr at startup
                         "Warning: 'theme' in language block is ignored..."
```

---

## Anti-Patterns

1. **Placeholder Options**: Don't define options that aren't implemented. They confuse users and bloat the codebase.

2. **Scattered Validation**: Don't validate in command handlers. Put validators in `OptionDef`.

3. **Duplicate Resolution**: Don't copy-paste `OptionResolver` chains. Use shared `Editor::resolve_option()`.

4. **Silent Config Errors**: Don't silently ignore invalid config. Warn users so they can fix.

5. **Multiple Access APIs**: Don't add more access patterns. Consolidate to `editor.option()` and `buffer.option()`.

---

## Success Criteria

- [x] Only actively-used options remain defined (TAB_WIDTH, THEME)
- [x] All validation centralized in option system via `OptionDef.validator`
- [x] Single resolution path via `Editor::resolve_option()`
- [x] Parse-time warnings for scope mismatches
- [x] All documentation updated
- [x] All tests passing
- [x] No clippy warnings (one allowed: type_complexity on validator fn type)

---

## Files Summary

| File | Changes |
|------|---------|
| `crates/registry/options/src/impls/*.rs` | Remove dead options, keep only indent.rs and theme.rs |
| `crates/registry/options/src/lib.rs` | Add validator field, validate_value(), update docs |
| `crates/registry/options/src/validators.rs` | New: standard validators |
| `crates/macro/src/option.rs` | Add validator field to generated code |
| `crates/api/src/capabilities.rs` | Remove hardcoded validation, delegate to central |
| `crates/api/src/editor/mod.rs` | Add resolve_option() method |
| `crates/api/src/buffer/mod.rs` | Delegate to Editor::resolve_option() |
| `crates/config/src/options.rs` | Add scope validation, return warnings |
| `crates/config/src/error.rs` | Add ConfigWarning type |
| `crates/term/src/main.rs` | Display config warnings |
