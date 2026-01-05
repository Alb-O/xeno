# Task 09A: Options Derive Macro Refactor

## Model Directive

This document specifies a major refactor of the options system from declarative macros to proc-macro derives. The goal is idiomatic Rust, reduced boilerplate, and compile-time type safety without runtime extraction.

**Scope**: Replace `option!` macro with `#[derive(Option)]`, consolidate parse utilities, add type-safe generic accessors, integrate option change hooks.

---

## Implementation Expectations

<mandatory_execution_requirements>

This is a **destroy-and-rebuild** refactor. When implementing:

1. Create new derive macro infrastructure first (can coexist)
2. Migrate options one category at a time with verification
3. Remove old macro only after all options migrated
4. Run `cargo build --workspace` after each phase
5. Run `cargo test --workspace` after completing each major cleanup

Unacceptable:
- Leaving both systems permanently coexisting
- Partial migrations that break the build
- Skipping verification steps

</mandatory_execution_requirements>

---

## Behavioral Constraints

<verbosity_and_scope_constraints>

- Each phase ends with a **cleanup checkpoint** where old code is removed
- Prefer editing existing files; new files only for new infrastructure
- Follow existing crate boundaries (proc macro in `xeno-macro`)
- Maintain backward compatibility at public API boundaries during transition

</verbosity_and_scope_constraints>

<design_freedom>

- The derive macro syntax can differ from the declarative macro
- Internal storage can change (e.g., `HashMap<&'static str, _>` -> `HashMap<OptionKey, _>`)
- New abstractions welcome when they reduce total linecount

</design_freedom>

---

## Architecture Overview

### Current State (Task 08A)

```
option!(tab_width, {
    kdl: "tab-width",
    type: Int,
    default: 4,
    scope: Buffer,
    description: "...",
});
```

Generates:
- `static OPT_TAB_WIDTH: OptionDef` in `OPTIONS` slice
- `pub const tab_width: OptionKey` handle

### Target State

```rust
#[derive(Option)]
#[option(kdl = "tab-width", scope = buffer)]
/// Number of spaces a tab character occupies for display.
pub static TAB_WIDTH: i64 = 4;
```

Generates:
- Same `OptionDef` registration
- Typed accessor: `keys::TAB_WIDTH` with compile-time type knowledge
- Type extracted from static's type annotation

### Key Improvements

| Aspect | Before | After |
|--------|--------|-------|
| Type declaration | `type: Int` string token | Rust type annotation `i64` |
| Default value | `default: 4` | Literal initializer `= 4` |
| Description | `description: "..."` | Doc comment `///` |
| Type safety | Runtime `as_int()` unwrap | Compile-time `T` bound |
| Accessor pattern | `option_int(key)` | `option::<i64>(key)` |

---

## Implementation Roadmap

### Phase 1: Derive Macro Infrastructure

**Objective**: Create the proc macro that parses `#[derive(Option)]` and generates registrations.

**Files**:
- `crates/macro/src/option.rs` (new)
- `crates/macro/src/lib.rs` (add export)

- [ ] 1.1 Create `crates/macro/src/option.rs` with `derive_option` proc macro
  - Parse `#[option(...)]` attributes
  - Extract type from static's type annotation
  - Generate `OptionDef` static and `OptionKey` constant
  - Handle doc comments as description

- [ ] 1.2 Add `Option` derive export to `crates/macro/src/lib.rs`
  - Wire up the new proc macro

- [ ] 1.3 Add typed key wrapper that carries type info
  - `TypedOptionKey<T>` that wraps `OptionKey` with phantom type
  - Enables `option::<T>(key)` to verify at compile time

- [ ] 1.4 Verify: `cargo build -p xeno-macro` passes

**Derive Macro Implementation Details**:

```rust
// Input
#[derive(Option)]
#[option(kdl = "tab-width", scope = buffer)]
/// Number of spaces a tab character occupies.
pub static TAB_WIDTH: i64 = 4;

// Generated
#[linkme::distributed_slice(OPTIONS)]
static __OPT_TAB_WIDTH: OptionDef = OptionDef {
    id: concat!(env!("CARGO_PKG_NAME"), "::TAB_WIDTH"),
    name: "TAB_WIDTH",
    kdl_key: "tab-width",
    description: "Number of spaces a tab character occupies.",
    value_type: OptionType::Int,
    default: || OptionValue::Int(4),
    scope: OptionScope::Buffer,
    priority: 0,
    source: RegistrySource::Crate(env!("CARGO_PKG_NAME")),
};

pub const TAB_WIDTH: TypedOptionKey<i64> = TypedOptionKey::new(&__OPT_TAB_WIDTH);
```

---

### Phase 2: Type-Safe Generic Accessor

**Objective**: Replace `option_int()`, `option_bool()`, `option_string()` with single generic `option::<T>()`.

**Files**:
- `crates/registry/options/src/lib.rs`
- `crates/registry/options/src/value.rs` (new, extract from lib.rs)
- `crates/registry/actions/src/editor_ctx/capabilities.rs`

- [ ] 2.1 Create sealed `FromOptionValue` trait
  ```rust
  pub trait FromOptionValue: Sealed {
      fn from_option(value: &OptionValue) -> Option<Self> where Self: Sized;
      fn option_type() -> OptionType;
  }

  impl FromOptionValue for i64 { ... }
  impl FromOptionValue for bool { ... }
  impl FromOptionValue for String { ... }
  ```

- [ ] 2.2 Add `TypedOptionKey<T>` wrapper
  ```rust
  pub struct TypedOptionKey<T: FromOptionValue> {
      inner: OptionKey,
      _marker: PhantomData<T>,
  }

  impl<T: FromOptionValue> TypedOptionKey<T> {
      pub fn get(&self) -> &'static OptionDef { self.inner.def() }
  }
  ```

- [ ] 2.3 Update `OptionAccess` trait with generic method
  ```rust
  pub trait OptionAccess {
      fn option<T: FromOptionValue>(&self, key: TypedOptionKey<T>) -> T;
  }
  ```

- [ ] 2.4 Keep old methods as deprecated shims during transition
  ```rust
  #[deprecated(note = "use option::<i64>(key) instead")]
  fn option_int(&self, key: OptionKey) -> i64 { ... }
  ```

- [ ] 2.5 Verify: `cargo build --workspace` passes with deprecation warnings

---

### Phase 3: Shared Parse Utility

**Objective**: Consolidate duplicate `parse_option_value` logic into single location.

**Files**:
- `crates/registry/options/src/parse.rs` (new)
- `crates/api/src/capabilities.rs` (remove duplicate)
- `crates/config/src/options.rs` (use shared)

- [ ] 3.1 Create `crates/registry/options/src/parse.rs`
  ```rust
  /// Parse string value into OptionValue based on type.
  pub fn parse_value(kdl_key: &str, value: &str) -> Result<OptionValue, OptionError> {
      let def = find_by_kdl(kdl_key)
          .ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
      parse_value_for_type(value, def.value_type)
  }

  /// Parse string into OptionValue for known type.
  pub fn parse_value_for_type(value: &str, ty: OptionType) -> Result<OptionValue, OptionError> {
      match ty {
          OptionType::Bool => parse_bool(value),
          OptionType::Int => parse_int(value),
          OptionType::String => Ok(OptionValue::String(value.to_string())),
      }
  }
  ```

- [ ] 3.2 Update `crates/api/src/capabilities.rs` to use shared parse
  - Remove `parse_option_value` function
  - Import from `xeno_registry::options::parse`

- [ ] 3.3 Update `crates/config/src/options.rs` to use shared parse
  - Remove inline parsing logic
  - Use `parse::parse_value_for_type`

- [ ] 3.4 Verify: `cargo test --workspace` passes

**CLEANUP CHECKPOINT 1**: Remove all duplicate parse code

---

### Phase 4: Migrate Options to Derive Macro

**Objective**: Convert all existing `option!` invocations to `#[derive(Option)]`.

**Strategy**: Migrate one file at a time, verify build after each.

- [ ] 4.1 Migrate `impls/indent.rs`
  ```rust
  // Before
  option!(tab_width, { kdl: "tab-width", type: Int, default: 4, ... });

  // After
  #[derive(Option)]
  #[option(kdl = "tab-width", scope = buffer)]
  /// Number of spaces a tab character occupies for display.
  pub static TAB_WIDTH: i64 = 4;
  ```
  - Verify: `cargo build -p xeno-registry-options`

- [ ] 4.2 Migrate `impls/display.rs`
  - `line_numbers`, `cursorline`, `wrap`, `colorcolumn`, `whitespace`
  - Verify build

- [ ] 4.3 Migrate `impls/scroll.rs`
  - `scroll_margin`, `sidescroll_margin`
  - Verify build

- [ ] 4.4 Migrate `impls/theme.rs`
  - `theme`
  - Verify build

- [ ] 4.5 Migrate `impls/behavior.rs`
  - `cursor_blink`, `confirm_quit`, `hidden_files`
  - Verify build

- [ ] 4.6 Migrate `impls/file.rs`
  - `auto_save`, `auto_save_delay`, `backup`, `encoding`, `line_ending`
  - Verify build

- [ ] 4.7 Migrate `impls/search.rs`
  - `ignorecase`, `smartcase`, `incsearch`, `hlsearch`, `wrapscan`
  - Verify build

- [ ] 4.8 Verify: Full `cargo test --workspace` passes

---

### Phase 5: Update All Access Sites

**Objective**: Update all code that accesses options to use typed keys.

- [ ] 5.1 Update render code in `crates/api/src/render/`
  - Find: `option_int(keys::`, `option_bool(keys::`
  - Replace: `option::<i64>(keys::`, `option::<bool>(keys::`

- [ ] 5.2 Update buffer code in `crates/api/src/buffer/`
  - Same pattern

- [ ] 5.3 Update window code in `crates/api/src/window/`
  - Same pattern

- [ ] 5.4 Update editor code in `crates/api/src/editor/`
  - Same pattern

- [ ] 5.5 Update actions in `crates/registry/actions/`
  - Same pattern

- [ ] 5.6 Verify: `cargo build --workspace` with no deprecation warnings

**CLEANUP CHECKPOINT 2**: Remove deprecated `option_int/bool/string` methods

---

### Phase 6: Remove Old Macro Infrastructure

**Objective**: Delete the declarative `option!` macro and related code.

- [ ] 6.1 Remove `crates/registry/options/src/macros.rs`
  - Delete the file entirely

- [ ] 6.2 Update `crates/registry/options/src/lib.rs`
  - Remove `mod macros;`
  - Remove `#[macro_export]` re-exports
  - Clean up imports

- [ ] 6.3 Update `keys` module to re-export from new locations
  - May need adjustment based on derive macro output location

- [ ] 6.4 Verify: `cargo build --workspace` passes
- [ ] 6.5 Verify: `cargo test --workspace` passes

**CLEANUP CHECKPOINT 3**: Old macro system fully removed

---

### Phase 7: Option Change Hooks

**Objective**: Integrate option changes with the hook system for reactive extensions.

**Files**:
- `crates/registry/hooks/src/events.rs`
- `crates/registry/commands/src/impls/set.rs`
- `crates/api/src/capabilities.rs`

- [ ] 7.1 Add option change event to `define_events!`
  ```rust
  define_events! {
      // ... existing events ...
      OptionChanged => "option:changed" {
          key: OptionStr,      // KDL key
          scope: OptionStr,    // "global" | "buffer"
      },
  }
  ```

- [ ] 7.2 Update `:set` command to emit hook
  ```rust
  ctx.emit(keys::option_set::call(&key, &value));
  // Also trigger hook
  ctx.trigger_hook(HookEvent::OptionChanged, OptionChangedData { key, scope: "global" });
  ```

- [ ] 7.3 Update `:setlocal` command similarly

- [ ] 7.4 Add example hook registration (in docs or test)
  ```rust
  hook!(on_option_changed, OptionChanged, |key, scope| {
      if key == "theme" {
          // Reload theme
      }
  });
  ```

- [ ] 7.5 Verify: `cargo test --workspace` passes

---

### Phase 8: Flatten Resolver (Optional Optimization)

**Objective**: Evaluate whether `OptionResolver` can be simplified.

**Analysis**: The resolver is created per-call and immediately dropped. Consider:

Option A: Keep resolver (current)
- Pro: Clear separation, easy to test
- Con: Allocation per resolution

Option B: Direct chain method
```rust
buffer.local_options
    .chain(&lang_store)
    .chain(&global_options)
    .get_or_default(key)
```

Option C: Inline resolution in `OptionAccess::option()`
- Pro: Zero allocation
- Con: Logic duplication if used elsewhere

- [ ] 8.1 Benchmark current resolver overhead
  - If negligible, keep as-is
  - If significant, implement Option B or C

- [ ] 8.2 If refactoring, update all resolution sites

- [ ] 8.3 Verify: `cargo test --workspace` passes

---

### Phase 9: Documentation and Cleanup

**Objective**: Update all documentation to reflect new system.

- [ ] 9.1 Update `AGENTS.md` options section
  - New derive syntax
  - Typed accessor pattern
  - Hook integration

- [ ] 9.2 Update `crates/registry/options/src/lib.rs` module docs
  - Examples with new syntax

- [ ] 9.3 Audit for dead code
  - Run `cargo +nightly udeps` or manual review
  - Remove any unused functions/types

- [ ] 9.4 Final verification
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `cargo clippy --workspace`

---

## Derive Macro Specification

### Attribute Syntax

```rust
#[derive(Option)]
#[option(
    kdl = "kebab-key",           // Required: KDL config key
    scope = global | buffer,     // Required: Option scope
    priority = 100,              // Optional: Sort order (default 0)
)]
/// Documentation becomes description.
pub static NAME: Type = default_value;
```

### Supported Types

| Rust Type | OptionType | OptionValue |
|-----------|------------|-------------|
| `i64` | `Int` | `OptionValue::Int(v)` |
| `bool` | `Bool` | `OptionValue::Bool(v)` |
| `String` | `String` | `OptionValue::String(v)` |
| `&'static str` | `String` | `OptionValue::String(v.into())` |

### Error Handling

The derive macro should produce compile errors for:
- Missing `#[option(...)]` attribute
- Missing required fields (`kdl`, `scope`)
- Unsupported type annotation
- Non-static item

---

## Anti-Patterns

1. **Leaving both macro systems**: The old `option!` macro must be fully removed once migration is complete. No permanent coexistence.

2. **String-based access internally**: All internal code should use `TypedOptionKey<T>`. String-based access only at config parsing boundary.

3. **Runtime type checking**: The generic accessor should be compile-time safe. No `unwrap()` on type extraction in hot paths.

4. **Scattered parse logic**: All option value parsing must go through `xeno_registry::options::parse`.

---

## Success Criteria

- [ ] All options defined using `#[derive(Option)]`
- [ ] Old `option!` macro deleted
- [ ] `option::<T>(key)` pattern used everywhere
- [ ] Single `parse_value` utility
- [ ] Option change hooks functional
- [ ] No deprecation warnings in build
- [ ] All tests passing
