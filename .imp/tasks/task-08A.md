# Task 08A: Unified Configuration System

## Summary

Design and implement a robust configuration system where option schemas are colocated with registry definitions, KDL keys are explicit (no guessing), and runtime access supports both global defaults and buffer-local overrides.

## Design Goals

1. **Colocated schema**: Options defined alongside their primary consumer (owned) or in central location (shared)
2. **Explicit KDL mapping**: Raw KDL strings in macros - what you write is what appears in config
3. **Type-safe access**: Typed keys for compile-time safety, strings only at boundaries
4. **Layered resolution**: Default -> Global config -> Language config -> Buffer-local
5. **Hot reload ready**: Architecture supports config file watching (implementation deferred)

## Design Decisions

### 1. Option Ownership Model

| Type | Location | Example |
|------|----------|---------|
| **Owned** | Colocated with consumer crate | `scrolloff` in scroll actions |
| **Shared** | Central `registry/options` | `tab_width`, `theme` |

Owned options use the same `option!` macro but live in the consumer crate.

### 2. KDL Schema in Macro

```rust
option!(tab_width, {
    kdl: "tab-width",           // Explicit KDL key (required)
    type: Int,                   // Int | Bool | String
    default: 4,
    scope: Buffer,               // Global | Buffer
    description: "Spaces per tab character",
});
```

The `kdl:` field is the source of truth - no automatic `snake_case` -> `kebab-case` conversion.

### 3. Access Patterns

```rust
// Global default (ignores buffer overrides)
let theme = options::global(options::keys::theme);

// Context-aware (buffer -> language -> global -> default)
let width = ctx.option(options::keys::tab_width);

// Direct buffer query
let width = buffer.option(options::keys::tab_width);
```

### 4. Resolution Order

```
1. Buffer-local override (set via :setlocal)
2. Language-specific config (from language "rust" { } block)
3. Global config (from options { } block)
4. Compile-time default (from option! macro)
```

### 5. Value Storage

```rust
// Runtime option store (per-buffer or global)
pub struct OptionStore {
    values: HashMap<&'static str, OptionValue>,  // KDL key -> value
}

// Type-safe accessor returns Option<T> matching declared type
impl OptionStore {
    pub fn get<T: FromOptionValue>(&self, key: OptionKey) -> Option<T>;
}
```

---

## Phase 1: Redesign Option Registry

### Task 1.1: Update `OptionDef` structure

**File**: `crates/registry/options/src/lib.rs`

```rust
/// Definition of a configurable option.
pub struct OptionDef {
    /// Unique identifier (e.g., "xeno_registry_options::tab_width").
    pub id: &'static str,
    /// Internal name for typed key references.
    pub name: &'static str,
    /// KDL configuration key (e.g., "tab-width").
    pub kdl_key: &'static str,
    /// Human-readable description.
    pub description: &'static str,
    /// Value type constraint.
    pub value_type: OptionType,
    /// Default value factory.
    pub default: fn() -> OptionValue,
    /// Application scope.
    pub scope: OptionScope,
    /// Priority for ordering (documentation, completion).
    pub priority: i16,
    /// Origin of definition.
    pub source: RegistrySource,
}

/// Typed handle to an option definition.
pub type OptionKey = Key<OptionDef>;
```

### Task 1.2: Update `option!` macro with KDL field

**File**: `crates/registry/options/src/macros.rs`

```rust
#[macro_export]
macro_rules! option {
    ($name:ident, {
        kdl: $kdl:literal,
        type: $type:ident,
        default: $default:expr,
        scope: $scope:ident,
        description: $desc:expr
        $(, priority: $priority:expr)?
        $(,)?
    }) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::OPTIONS)]
            static [<OPT_ $name:upper>]: $crate::OptionDef = $crate::OptionDef {
                id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
                name: stringify!($name),
                kdl_key: $kdl,
                description: $desc,
                value_type: $crate::OptionType::$type,
                default: || $crate::OptionValue::$type($default),
                scope: $crate::OptionScope::$scope,
                priority: $crate::__opt_priority!($($priority)?),
                source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
            };

            #[doc = concat!("Typed handle for the `", stringify!($name), "` option.")]
            #[allow(non_upper_case_globals)]
            pub const $name: $crate::OptionKey = $crate::OptionKey::new(&[<OPT_ $name:upper>]);
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_priority {
    () => { 0 };
    ($val:expr) => { $val };
}
```

### Task 1.3: Add lookup functions

**File**: `crates/registry/options/src/lib.rs`

```rust
/// Finds an option by its KDL key.
pub fn find_by_kdl(kdl_key: &str) -> Option<&'static OptionDef> {
    OPTIONS.iter().find(|o| o.kdl_key == kdl_key)
}

/// Finds an option by internal name.
pub fn find_by_name(name: &str) -> Option<&'static OptionDef> {
    OPTIONS.iter().find(|o| o.name == name)
}

/// Returns all options sorted by KDL key.
pub fn all_sorted() -> impl Iterator<Item = &'static OptionDef> {
    let mut opts: Vec<_> = OPTIONS.iter().collect();
    opts.sort_by_key(|o| o.kdl_key);
    opts.into_iter()
}

/// Validates a KDL key exists and value matches expected type.
pub fn validate(kdl_key: &str, value: &OptionValue) -> Result<(), OptionError> {
    let def = find_by_kdl(kdl_key).ok_or(OptionError::UnknownOption(kdl_key.to_string()))?;
    if !value.matches_type(def.value_type) {
        return Err(OptionError::TypeMismatch {
            option: kdl_key.to_string(),
            expected: def.value_type,
            got: value.type_name(),
        });
    }
    Ok(())
}
```

### Task 1.4: Add `keys` module for typed handles

**File**: `crates/registry/options/src/lib.rs`

```rust
/// Typed handles for built-in options.
pub mod keys {
    pub use crate::impls::behavior::*;
    pub use crate::impls::display::*;
    pub use crate::impls::file::*;
    pub use crate::impls::indent::*;
    pub use crate::impls::scroll::*;
    pub use crate::impls::search::*;
    pub use crate::impls::theme::*;
}
```

---

## Phase 2: Migrate Existing Options

### Task 2.1: Update indent options

**File**: `crates/registry/options/src/impls/indent.rs`

```rust
use crate::option;

option!(tab_width, {
    kdl: "tab-width",
    type: Int,
    default: 4,
    scope: Buffer,
    description: "Number of spaces a tab character occupies for display",
});

option!(indent_width, {
    kdl: "indent-width",
    type: Int,
    default: 4,
    scope: Buffer,
    description: "Number of spaces per indentation level",
});

option!(use_tabs, {
    kdl: "use-tabs",
    type: Bool,
    default: false,
    scope: Buffer,
    description: "Use tabs instead of spaces for indentation",
});
```

### Task 2.2: Update display options

**File**: `crates/registry/options/src/impls/display.rs`

```rust
option!(line_numbers, {
    kdl: "line-numbers",
    type: String,
    default: "absolute".into(),
    scope: Global,
    description: "Line number style: absolute, relative, hybrid, none",
});

option!(cursorline, {
    kdl: "cursorline",
    type: Bool,
    default: true,
    scope: Global,
    description: "Highlight the line containing the cursor",
});

option!(wrap, {
    kdl: "wrap",
    type: Bool,
    default: false,
    scope: Buffer,
    description: "Wrap long lines instead of horizontal scrolling",
});
```

### Task 2.3: Update scroll options

**File**: `crates/registry/options/src/impls/scroll.rs`

```rust
option!(scrolloff, {
    kdl: "scrolloff",
    type: Int,
    default: 5,
    scope: Global,
    description: "Minimum lines to keep above/below cursor when scrolling",
});

option!(sidescrolloff, {
    kdl: "sidescrolloff",
    type: Int,
    default: 8,
    scope: Global,
    description: "Minimum columns to keep left/right of cursor",
});
```

### Task 2.4: Update theme option

**File**: `crates/registry/options/src/impls/theme.rs`

```rust
option!(theme, {
    kdl: "theme",
    type: String,
    default: "gruvbox".into(),
    scope: Global,
    description: "Active color theme name",
});
```

### Task 2.5: Update remaining option files

Apply same pattern to:
- `impls/behavior.rs`
- `impls/file.rs`
- `impls/search.rs`

---

## Phase 3: Create Option Store

### Task 3.1: Create `OptionStore` type

**File**: `crates/registry/options/src/store.rs`

```rust
use std::collections::HashMap;
use crate::{OptionDef, OptionKey, OptionValue, find_by_kdl};

/// Runtime storage for option values.
#[derive(Debug, Clone, Default)]
pub struct OptionStore {
    /// Values keyed by KDL key for config parsing.
    values: HashMap<&'static str, OptionValue>,
}

impl OptionStore {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets an option value by typed key.
    pub fn set(&mut self, key: OptionKey, value: OptionValue) {
        self.values.insert(key.def().kdl_key, value);
    }

    /// Sets an option value by KDL key (for config parsing).
    pub fn set_by_kdl(&mut self, kdl_key: &str, value: OptionValue) -> Result<(), OptionError> {
        let def = find_by_kdl(kdl_key)
            .ok_or_else(|| OptionError::UnknownOption(kdl_key.to_string()))?;
        self.values.insert(def.kdl_key, value);
        Ok(())
    }

    /// Gets an option value, returning None if not set.
    pub fn get(&self, key: OptionKey) -> Option<&OptionValue> {
        self.values.get(key.def().kdl_key)
    }

    /// Gets typed value with automatic conversion.
    pub fn get_int(&self, key: OptionKey) -> Option<i64> {
        self.get(key).and_then(|v| v.as_int())
    }

    pub fn get_bool(&self, key: OptionKey) -> Option<bool> {
        self.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_string(&self, key: OptionKey) -> Option<&str> {
        self.get(key).and_then(|v| v.as_str())
    }

    /// Merges another store into this one (other wins on conflict).
    pub fn merge(&mut self, other: &OptionStore) {
        for (k, v) in &other.values {
            self.values.insert(k, v.clone());
        }
    }

    /// Returns iterator over all set values.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &OptionValue)> {
        self.values.iter().map(|(k, v)| (*k, v))
    }
}
```

### Task 3.2: Add error types

**File**: `crates/registry/options/src/error.rs`

```rust
use crate::OptionType;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum OptionError {
    #[error("unknown option: {0}")]
    UnknownOption(String),

    #[error("type mismatch for option '{option}': expected {expected:?}, got {got}")]
    TypeMismatch {
        option: String,
        expected: OptionType,
        got: &'static str,
    },

    #[error("invalid value for option '{option}': {reason}")]
    InvalidValue {
        option: String,
        reason: String,
    },
}
```

---

## Phase 4: Layered Resolution

### Task 4.1: Create `OptionResolver` trait

**File**: `crates/registry/options/src/resolver.rs`

```rust
use crate::{OptionKey, OptionStore, OptionValue};

/// Resolves option values through a layered hierarchy.
pub struct OptionResolver<'a> {
    /// Buffer-local overrides (highest priority).
    buffer_local: Option<&'a OptionStore>,
    /// Language-specific settings.
    language: Option<&'a OptionStore>,
    /// Global user configuration.
    global: Option<&'a OptionStore>,
    // Defaults come from OptionDef
}

impl<'a> OptionResolver<'a> {
    pub fn new() -> Self {
        Self {
            buffer_local: None,
            language: None,
            global: None,
        }
    }

    pub fn with_buffer(mut self, store: &'a OptionStore) -> Self {
        self.buffer_local = Some(store);
        self
    }

    pub fn with_language(mut self, store: &'a OptionStore) -> Self {
        self.language = Some(store);
        self
    }

    pub fn with_global(mut self, store: &'a OptionStore) -> Self {
        self.global = Some(store);
        self
    }

    /// Resolves an option through the hierarchy.
    pub fn resolve(&self, key: OptionKey) -> OptionValue {
        // Buffer-local first
        if let Some(store) = self.buffer_local {
            if let Some(v) = store.get(key) {
                return v.clone();
            }
        }
        // Language config second
        if let Some(store) = self.language {
            if let Some(v) = store.get(key) {
                return v.clone();
            }
        }
        // Global config third
        if let Some(store) = self.global {
            if let Some(v) = store.get(key) {
                return v.clone();
            }
        }
        // Fall back to default
        (key.def().default)()
    }

    /// Typed resolution helpers.
    pub fn resolve_int(&self, key: OptionKey) -> i64 {
        self.resolve(key).as_int().unwrap_or_else(|| {
            (key.def().default)().as_int().unwrap()
        })
    }

    pub fn resolve_bool(&self, key: OptionKey) -> bool {
        self.resolve(key).as_bool().unwrap_or_else(|| {
            (key.def().default)().as_bool().unwrap()
        })
    }

    pub fn resolve_string(&self, key: OptionKey) -> String {
        self.resolve(key).as_str().map(|s| s.to_string()).unwrap_or_else(|| {
            (key.def().default)().as_str().unwrap().to_string()
        })
    }
}
```

---

## Phase 5: Update Config Parsing

### Task 5.1: Update `OptionsConfig` to use `OptionStore`

**File**: `crates/config/src/options.rs`

```rust
use xeno_registry::options::{OptionStore, OptionValue, find_by_kdl, OptionError};

/// Parse options from KDL, validating against registry.
pub fn parse_options_node(node: &KdlNode) -> Result<OptionStore, ConfigError> {
    let mut store = OptionStore::new();

    let Some(children) = node.children() else {
        return Ok(store);
    };

    for opt_node in children.nodes() {
        let kdl_key = opt_node.name().value();

        // Validate option exists
        let def = find_by_kdl(kdl_key).ok_or_else(|| {
            ConfigError::UnknownOption {
                key: kdl_key.to_string(),
                suggestion: suggest_option(kdl_key),
            }
        })?;

        if let Some(entry) = opt_node.entries().first() {
            let value = entry.value();
            let opt_value = parse_kdl_value(value, def.value_type)?;
            store.set_by_kdl(kdl_key, opt_value)?;
        }
    }

    Ok(store)
}

fn suggest_option(key: &str) -> Option<String> {
    // Fuzzy match against known KDL keys
    xeno_registry::options::all_sorted()
        .map(|o| o.kdl_key)
        .min_by_key(|k| strsim::levenshtein(key, k))
        .filter(|k| strsim::levenshtein(key, k) <= 3)
        .map(|s| s.to_string())
}
```

### Task 5.2: Update `Config` struct

**File**: `crates/config/src/lib.rs`

```rust
use xeno_registry::options::OptionStore;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub theme: Option<ParsedTheme>,
    pub keys: Option<KeysConfig>,
    /// Global option overrides.
    pub options: OptionStore,
    /// Per-language option overrides.
    pub languages: HashMap<String, OptionStore>,
}
```

---

## Phase 6: Editor Integration

### Task 6.1: Add option stores to Editor

**File**: `crates/api/src/editor/mod.rs`

```rust
use xeno_registry::options::OptionStore;

pub struct Editor {
    // ... existing fields ...
    
    /// Global user configuration options.
    global_options: OptionStore,
    /// Per-language option overrides.
    language_options: HashMap<String, OptionStore>,
}
```

### Task 6.2: Add option store to Buffer

**File**: `crates/api/src/buffer/mod.rs`

```rust
use xeno_registry::options::OptionStore;

pub struct Buffer {
    // ... existing fields ...
    
    /// Buffer-local option overrides (from :setlocal).
    local_options: OptionStore,
}
```

### Task 6.3: Add `option()` method to Buffer

**File**: `crates/api/src/buffer/mod.rs`

```rust
impl Buffer {
    /// Resolves an option for this buffer.
    pub fn option(&self, key: OptionKey, editor: &Editor) -> OptionValue {
        let language_store = self.file_type()
            .and_then(|ft| editor.language_options.get(ft));
        
        OptionResolver::new()
            .with_buffer(&self.local_options)
            .with_language(language_store.unwrap_or(&OptionStore::new()))
            .with_global(&editor.global_options)
            .resolve(key)
    }

    /// Typed helper for int options.
    pub fn option_int(&self, key: OptionKey, editor: &Editor) -> i64 {
        self.option(key, editor).as_int().unwrap_or_else(|| {
            (key.def().default)().as_int().unwrap()
        })
    }
}
```

### Task 6.4: Add global accessor

**File**: `crates/registry/options/src/lib.rs`

```rust
use std::sync::OnceLock;

static GLOBAL_OPTIONS: OnceLock<OptionStore> = OnceLock::new();

/// Initialize global options from config.
pub fn init_global(store: OptionStore) {
    let _ = GLOBAL_OPTIONS.set(store);
}

/// Get a global option value (ignoring buffer overrides).
pub fn global(key: OptionKey) -> OptionValue {
    GLOBAL_OPTIONS
        .get()
        .and_then(|s| s.get(key).cloned())
        .unwrap_or_else(|| (key.def().default)())
}
```

---

## Phase 7: Capability Integration

### Task 7.1: Add `OptionAccess` capability

**File**: `crates/registry/actions/src/editor_ctx/capabilities.rs`

```rust
use xeno_registry::options::{OptionKey, OptionValue};

/// Access to configuration options.
pub trait OptionAccess {
    /// Resolves an option for the current context (buffer-aware).
    fn option(&self, key: OptionKey) -> OptionValue;
    
    /// Typed helpers.
    fn option_int(&self, key: OptionKey) -> i64 {
        self.option(key).as_int().unwrap_or(0)
    }
    
    fn option_bool(&self, key: OptionKey) -> bool {
        self.option(key).as_bool().unwrap_or(false)
    }
    
    fn option_string(&self, key: OptionKey) -> String {
        self.option(key).as_str().unwrap_or("").to_string()
    }
}
```

### Task 7.2: Implement for `EditorContext`

Wire up the capability to use the resolver with current buffer context.

---

## Phase 8: Commands

### Task 8.1: Implement `:set` command

**File**: `crates/registry/commands/src/impls/set.rs`

```rust
command!(set, {
    description: "Set an option globally",
    aliases: &["se"],
}, |ctx| async move {
    let args = ctx.args;
    if args.is_empty() {
        // Show all options
        return Ok(CommandOutcome::Ok);
    }
    
    // Parse "option=value" or "option value"
    let (key, value) = parse_set_args(args)?;
    
    let def = find_by_kdl(key)
        .ok_or_else(|| CommandError::InvalidArgument(format!("unknown option: {key}")))?;
    
    let parsed = parse_value_for_type(value, def.value_type)?;
    
    // Apply to global store
    ctx.editor.set_global_option(def.kdl_key, parsed);
    
    Ok(CommandOutcome::Ok)
});
```

### Task 8.2: Implement `:setlocal` command

```rust
command!(setlocal, {
    description: "Set an option for current buffer only",
    aliases: &["setl"],
}, |ctx| async move {
    // Same parsing as :set
    // Apply to buffer.local_options instead
    Ok(CommandOutcome::Ok)
});
```

---

## Phase 9: Remove Old Config Code

### Task 9.1: Remove `OptionsConfig` wrapper

The old `HashMap<String, OptionValue>` wrapper is replaced by `OptionStore`.

### Task 9.2: Remove old `crates/config/src/options.rs` parsing

Replace with new validation-aware parsing.

### Task 9.3: Update all option access sites

Find all places that access options and update to use typed keys:

```rust
// Before
let width = config.options.get("tab-width").and_then(|v| v.as_int()).unwrap_or(4);

// After  
let width = buffer.option_int(options::keys::tab_width, editor);
```

---

## Phase 10: Documentation & Testing

### Task 10.1: Update AGENTS.md

Add options to registry table, document the new access patterns.

### Task 10.2: Add unit tests

- Test `OptionStore` set/get
- Test `OptionResolver` layer precedence
- Test config parsing validation
- Test unknown option errors with suggestions

### Task 10.3: Add integration test

Test full flow: KDL config -> parse -> store -> resolve with buffer context.

---

## Task Checklist

### Phase 1: Redesign Option Registry
- [x] Update `OptionDef` with `kdl_key` field
- [x] Update `option!` macro with `kdl:` parameter
- [x] Add `find_by_kdl()`, `find_by_name()`, `validate()` functions
- [x] Add `keys` module exporting typed handles
- [x] Add `OptionKey` type alias

### Phase 2: Migrate Existing Options
- [x] Update `impls/indent.rs` (tab_width, indent_width, use_tabs)
- [x] Update `impls/display.rs` (line_numbers, cursorline, wrap)
- [x] Update `impls/scroll.rs` (scrolloff, sidescrolloff)
- [x] Update `impls/theme.rs` (theme)
- [x] Update `impls/behavior.rs`
- [x] Update `impls/file.rs`
- [x] Update `impls/search.rs`

### Phase 3: Create Option Store
- [x] Create `crates/registry/options/src/store.rs`
- [x] Implement `OptionStore` with typed accessors
- [x] ~~Create `crates/registry/options/src/error.rs`~~ (error types defined inline in lib.rs)
- [x] Add error types (`UnknownOption`, `TypeMismatch`, `InvalidValue`)

### Phase 4: Layered Resolution
- [x] Create `crates/registry/options/src/resolver.rs`
- [x] Implement `OptionResolver` with layer chain
- [x] Add typed resolution helpers

### Phase 5: Update Config Parsing
- [x] Update `crates/config/src/options.rs` to validate against registry
- [x] Add `suggest_option()` fuzzy matching
- [x] Update `Config` struct to use `OptionStore`
- [x] Update `LanguageConfig` to use `OptionStore`

### Phase 6: Editor Integration
- [x] Add `global_options: OptionStore` to `Editor`
- [x] Add `language_options: HashMap<String, OptionStore>` to `Editor`
- [x] Add `local_options: OptionStore` to `Buffer`
- [x] Implement `Buffer::option()` method
- [x] Add global `init_global()` and `global()` functions

### Phase 7: Capability Integration
- [x] Add `OptionAccess` trait to capabilities
- [x] Implement for `EditorContext`

### Phase 8: Commands
- [x] Implement `:set` command
- [x] Implement `:setlocal` command

### Phase 9: Remove Old Code
- [x] Remove old `OptionsConfig` type
- [x] Remove old config parsing code
- [x] Update all option access sites to use typed keys

### Phase 10: Documentation & Testing
- [x] Update AGENTS.md with options documentation
- [x] Add unit tests for `OptionStore`
- [x] Add unit tests for `OptionResolver`
- [x] Add integration test for full config flow
- [x] Verify hot reload architecture is compatible (no impl yet)

---

## Future Extensions

### Owned Options (Colocated)

Once core system is working, options can be defined in consumer crates:

```rust
// In crates/registry/actions/src/impls/scroll.rs
use xeno_registry::options::option;

option!(scrolloff, {
    kdl: "scrolloff",
    type: Int,
    default: 5,
    scope: Global,
    description: "Minimum lines above/below cursor",
});

// Used in same file
action!(scroll_down, { ... }, |ctx| {
    let margin = ctx.option_int(scrolloff);  // Direct reference
    // ...
});
```

### Validation Constraints

```rust
option!(scrolloff, {
    kdl: "scrolloff",
    type: Int,
    default: 5,
    scope: Global,
    description: "...",
    validate: |v| v >= 0 && v <= 999,
});
```

### Config File Hot Reload

```rust
// Watch for changes, re-parse, update stores
config_watcher.on_change(|new_config| {
    editor.global_options = new_config.options;
    editor.language_options = new_config.languages;
    // Trigger re-render
});
```

### KDL Schema Generation

Generate KDL schema for editor config validation/completion:

```kdl
// Auto-generated from OPTIONS slice
options {
    tab-width (type="int" default=4 scope="buffer") "Spaces per tab"
    theme (type="string" default="gruvbox" scope="global") "Color theme"
}
```

---

## Notes

- **No automatic key transformation**: `tab_width` in code, `tab-width` in KDL - both explicitly specified
- **Type safety internal, strings at boundary**: Typed keys inside codebase, strings only for config parsing
- **Resolver is stateless**: Create per-resolution, no caching (simple, correct first)
- **Buffer-local is explicit**: Only set via `:setlocal`, not automatically from language config
