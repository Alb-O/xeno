# Xeno Registry Refactor Plan

## Background

The codebase recently removed `linkme` (distributed slices) and now uses **manual static arrays** for registry collection. This creates maintenance burden and merge conflict potential across 12 registry types with ~300 total static items.

## Current State Analysis

### Registry Inventory

| Registry | Count | Location | Pain Level |
|----------|-------|----------|------------|
| Actions | 97 | `crates/registry/actions/src/lib.rs:68-165` | High |
| Keybindings | 98 sets | `crates/registry/actions/src/keybindings.rs:28-125` | High |
| Commands | 18 | `crates/registry/commands/src/lib.rs:232-249` | Medium |
| Motions | 22 | `crates/registry/motions/src/lib.rs:114-136` | Medium |
| Text Objects | 13 | `crates/registry/textobj/src/lib.rs:115-129` | Low |
| Gutters | 4 | `crates/registry/gutter/src/lib.rs:127-132` | Low |
| Hooks | 3 | `crates/registry/hooks/src/lib.rs:198-202` | Low |
| Statusline | 7 | `crates/registry/statusline/src/lib.rs:103-111` | Low |
| Options | 5 | `crates/registry/options/src/lib.rs:376-382` | Low |
| Themes | 1 | `crates/registry/themes/src/lib.rs:311` | Low |
| Notifications | 50+ | `crates/registry/notifications/src/` | Medium |
| Editor Commands | 7 | `crates/editor/src/commands/mod.rs:65-79` | Low |

### Current Pattern Problems

1. **Manual array maintenance**: Every `action!(foo, ...)` requires adding `&impls::module::ACTION_foo` to central array
2. **Linear lookup**: `find_action` iterates all ~100 entries on every call
3. **Merge conflicts**: Central arrays are conflict magnets
4. **No compile-time validation**: Can define static but forget to register it
5. **Pattern duplication**: All 12 registries duplicate this boilerplate

## Recommended Architecture (from ChatGPT collaboration)

### Why `inventory` over `linkme`

- `inventory` is explicitly designed for "no central list" pattern
- Avoids cross-crate linker section issues that caused linkme removal
- `submit!` is declarative, all submissions "take effect simultaneously"
- More robust across crate boundaries than distributed slices

### Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    action! macro                             │
│  - Generates ACTION_<name>: ActionDef static                │
│  - Generates <name>: ActionKey const                        │
│  - Generates KEYBINDINGS_<name> via parse_keybindings!      │
│  - Emits inventory::submit! { ActionReg(&ACTION_<name>) }   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              inventory::collect!(ActionReg)                  │
│  - Collects all ActionReg submissions at link time          │
│  - No manual array needed                                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│           ACTION_INDEX: LazyLock<Mutex<HashMap>>            │
│  - Built once on first access                               │
│  - Keys: name, aliases, id                                  │
│  - O(1) lookup via find_action()                            │
│  - Runtime registration updates same map                    │
└─────────────────────────────────────────────────────────────┘
```

---

## Implementation Tasks

### Phase 1: Actions Registry Migration (Proof of Concept)

- [ ] **1.1** Add `inventory = "0.3"` to `crates/registry/actions/Cargo.toml`

- [ ] **1.2** Add wrapper type and collect in `lib.rs`:
  ```rust
  pub struct ActionReg(pub &'static ActionDef);
  inventory::collect!(ActionReg);  // must be at module scope
  ```

- [ ] **1.3** Patch `action!` macro in `macros.rs` to emit `submit!`:
  ```rust
  // Add after ACTION_<name> definition, before closing paste!
  inventory::submit! {
      $crate::ActionReg(&[<ACTION_ $name>])
  }
  ```

- [ ] **1.4** Implement `LazyLock<Mutex<HashMap>>` index in `lib.rs`:
  ```rust
  static ACTION_INDEX: LazyLock<Mutex<HashMap<&'static str, &'static ActionDef>>> =
      LazyLock::new(|| {
          let mut map = HashMap::new();
          for reg in inventory::iter::<ActionReg> {
              let def = reg.0;
              insert_action(&mut map, def);
          }
          Mutex::new(map)
      });

  fn insert_action(map: &mut HashMap<&'static str, &'static ActionDef>, def: &'static ActionDef) {
      map.insert(def.name(), def);
      for &alias in def.aliases() {
          map.insert(alias, def);
      }
      map.insert(def.id(), def);
  }
  ```

- [ ] **1.5** Replace `find_action` with O(1) lookup:
  ```rust
  pub fn find_action(name: &str) -> Option<&'static ActionDef> {
      ACTION_INDEX.lock().expect("action index lock poisoned").get(name).copied()
  }
  ```

- [ ] **1.6** Update `register_action` to use same index:
  ```rust
  pub fn register_action(def: &'static ActionDef) {
      let mut map = ACTION_INDEX.lock().expect("action index lock poisoned");
      if let Some(existing) = map.get(def.name()) {
          if !std::ptr::eq(*existing, def) {
              eprintln!("warn: action name collision: {}", def.name());
              // last-write-wins for now
          }
      }
      insert_action(&mut map, def);
  }
  ```

- [ ] **1.7** Implement sorted `all_actions()` iterator:
  ```rust
  static ALL_ACTIONS: LazyLock<Vec<&'static ActionDef>> = LazyLock::new(|| {
      let mut actions: Vec<_> = inventory::iter::<ActionReg>().map(|r| r.0).collect();
      // Include runtime extras if any
      if let Some(extras) = EXTRA_ACTIONS.get() {
          actions.extend(extras.lock().unwrap().iter().copied());
      }
      actions.sort_by_key(|a| a.name());
      actions
  });

  pub fn all_actions() -> impl Iterator<Item = &'static ActionDef> {
      ALL_ACTIONS.iter().copied()
  }
  ```

- [ ] **1.8** Delete the manual `ACTIONS: &[&ActionDef] = &[...]` array (97 lines)

- [ ] **1.9** Verify keybindings collection still works (derives from same macro)

- [ ] **1.10** Run tests: `cargo test -p xeno-registry-actions`

### Phase 2: Keybindings Index

- [ ] **2.1** Implement `KEYBINDING_INDEX: LazyLock<HashMap>` for fast key lookup

- [ ] **2.2** Remove manual `KEYBINDING_SETS` array

- [ ] **2.3** Update `KEYBINDINGS: LazyLock<Vec<KeyBindingDef>>` to use inventory iteration

### Phase 3: Generic Registry Infrastructure

- [ ] **3.1** Define `RegistryItem` trait in `crates/registry/core`:
  ```rust
  pub trait RegistryItem {
      fn id(&self) -> &'static str;
      fn name(&self) -> &'static str;
      fn aliases(&self) -> &'static [&'static str];
      fn priority(&self) -> i16;
  }
  ```

- [ ] **3.2** Create generic `Registry<T: RegistryItem>` struct:
  ```rust
  pub struct Registry<T: RegistryItem> {
      index: LazyLock<Mutex<HashMap<&'static str, &'static T>>>,
      items: LazyLock<Vec<&'static T>>,
  }
  ```

- [ ] **3.3** Implement common methods: `find()`, `register()`, `all()`, `iter()`

- [ ] **3.4** Add collision detection and diagnostics

### Phase 4: Roll Out to Remaining Registries

- [ ] **4.1** Migrate Commands registry (18 items)
- [ ] **4.2** Migrate Motions registry (22 items)
- [ ] **4.3** Migrate TextObjects registry (13 items)
- [ ] **4.4** Migrate Gutters registry (4 items)
- [ ] **4.5** Migrate Hooks registry (3 items)
- [ ] **4.6** Migrate Statusline registry (7 items)
- [ ] **4.7** Migrate Options registry (5 items)
- [ ] **4.8** Migrate Themes registry (1 item)
- [ ] **4.9** Migrate Notifications registry (50+ items)
- [ ] **4.10** Migrate EditorCommands registry (7 items)

### Phase 5: Optional Optimizations

- [ ] **5.1** Evaluate PHF (perfect hashing) for builtin lookup if profiling shows need
  - `phf` crate for compile-time perfect hash tables
  - `hashify` for proc-macro approach (tiny maps <500 entries)
  - Only worth it if string→def lookup is hot path

- [ ] **5.2** Consider two-tier resolver if plugins become common:
  - Tier 1: Perfect hash table for builtins
  - Tier 2: Runtime HashMap for plugins

- [ ] **5.3** Add `cargo xtask check-registry` for CI validation:
  - Unique ids
  - No alias collisions
  - Priority invariants

---

## Technical Notes

### Cross-Crate Safety

Action definitions in `crates/registry/actions/src/impls/*.rs` are in the **same crate** as `lib.rs` where `collect!` is called. This is safe - inventory issues arise when `submit!` is in a different crate than `collect!`.

### Iteration Order

`inventory` iteration order is **not guaranteed**. Always sort when building indexes or help output. Sort by `priority` then `name` for deterministic behavior.

### LazyLock Poisoning

If the init closure panics, `LazyLock` becomes poisoned. This is acceptable for registry builds - fail fast and loud on startup rather than hiding errors.

### Runtime Plugin Architecture

Keep `OnceLock<Mutex<Vec<...>>>` pattern for runtime-loaded plugins. Treat as "overlay" merged into the same index. Plugins must register before first use, or use RwLock and rebuild index on add.

---

## References

- [inventory crate docs](https://docs.rs/inventory)
- [phf crate docs](https://docs.rs/phf)
- [hashify crate docs](https://docs.rs/hashify)
- [std::sync::LazyLock](https://doc.rust-lang.org/std/sync/struct.LazyLock.html)
- Commit history: "phase 4: remove linkme"
