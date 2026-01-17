# Xeno Scale Plan: Registry Migration + LSP Hardening + Architectural Priorities

This document is a multi-phase, checkbox-driven plan to:

1. Migrate registries from `linkme::distributed_slice` to explicit registration while **keeping definition macros**
2. Fix high-impact LSP sync issues (content cloning, version discipline)
3. Add a few scale-oriented architectural improvements (budgets, observability, determinism)

______________________________________________________________________

## Guiding Principles

- **Definitions stay static and ergonomic.** Macros like `action!`, `command!`, `gutter!` keep doing (1) define `*_Def` values.
- **Registration becomes explicit.** A small set of plugin registration functions wires builtins together in one navigable place.
- **Behavior is unchanged until explicitly migrated.** Use an adapter phase so you can migrate one registry type at a time.
- **Scale is protected by guardrails.** Collision checks, diagnostics, and perf counters become non-optional.

______________________________________________________________________

## Phase Overview and Dependencies

- **Phase 0 — Baseline & Guardrails**: inventory, tests, CI checks (no behavior changes)
- **Phase 1 — New Explicit Registration Infrastructure (Parallel)**: `RegistryBuilder`, `XenoPlugin`, adapters that ingest distributed slices
- **Phase 2 — Macro Split (Define-only macros)**: macros stop calling `linkme`, only define statics
- **Phase 3 — Registry Migration (One registry at a time)**: actions → commands → motions → gutters → handlers → keymaps
- **Phase 4 — Remove linkme**: delete distributed slices + macros + dependency, enforce "no linkme"
- **Phase 5 — Scale Improvements**: LSP clone removal, debounce discipline, version reconciliation, budgets + observability

**Critical path:** Phase 1 → Phase 2 → Phase 3 → Phase 4
**Parallelizable:** LSP improvements (Phase 5) can run in parallel with registry migration after Phase 0.

______________________________________________________________________

# Phase 0 — Baseline & Guardrails

### Goals

- Establish "known good" behavior and metrics before structural changes.

### Tasks

- [x] **Registry inventory script / report**: list counts and IDs per registry type (actions/commands/motions/gutters/etc.)
- [x] **Collision audit**: verify unique `RegistryMeta.id` across each registry type (even before migration)
- [x] **Smoke tests** (manual checklist):
  - [x] Can start editor
  - [x] Commands resolve by `name` and `aliases`
  - [x] Keybindings work
  - [x] LSP connects and produces diagnostics
- [x] **Add CI lint** to prevent new registries from silently proliferating:
  - [x] enforce naming pattern for `meta.id` (e.g. `core.save`, `lsp.hover`)
  - [ ] enforce max alias count or format (optional)
- [x] **Add perf counters** (even temporary):
  - [x] LSP flush: count full-sync vs incremental
  - [x] LSP: bytes of document text cloned per tick

### Risks

- *Risk:* Inventory tooling takes time.
  *Mitigation:* Start with runtime logging (counts + first 10 IDs) if needed.

______________________________________________________________________

# Phase 1 — Explicit Registration Infrastructure (Parallel, No Behavior Change)

### Goals

- Introduce explicit registration without deleting `distributed_slice` yet.
- Build a `RegistryBuilder` that can ingest either:
  - explicit registrations, or
  - legacy distributed slices (adapter)

### New Core Types (fits your `RegistryMeta` + `RegistrySource`)

```rust
// crates/registry/core/src/plugin.rs
pub trait XenoPlugin {
    const ID: &'static str;
    fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError>;
}
```

```rust
// crates/registry/core/src/builder.rs
use std::collections::HashMap;

#[derive(Debug)]
pub enum RegistryError {
    DuplicateId {
        kind: &'static str,
        id: &'static str,
        first: RegistrySource,
        second: RegistrySource,
    },
    DuplicateName {
        kind: &'static str,
        name: &'static str,
        first_id: &'static str,
        second_id: &'static str,
    },
    DuplicateAlias {
        kind: &'static str,
        alias: &'static str,
        first_id: &'static str,
        second_id: &'static str,
    },
}

pub struct RegistryBuilder {
    actions: HashMap<&'static str, &'static ActionDef>,
    commands: HashMap<&'static str, &'static CommandDef>,
    // motions, gutters, handlers, keymaps...
}

impl RegistryBuilder {
    pub fn new() -> Self { Self { actions: HashMap::new(), commands: HashMap::new() } }

    pub fn register_action(&mut self, def: &'static ActionDef) -> Result<(), RegistryError> {
        let id = def.meta.id;
        if let Some(prev) = self.actions.insert(id, def) {
            return Err(RegistryError::DuplicateId {
                kind: "action",
                id,
                first: prev.meta.source,
                second: def.meta.source,
            });
        }
        Ok(())
    }

    pub fn register_command(&mut self, def: &'static CommandDef) -> Result<(), RegistryError> {
        let id = def.meta.id;
        if let Some(prev) = self.commands.insert(id, def) {
            return Err(RegistryError::DuplicateId {
                kind: "command",
                id,
                first: prev.meta.source,
                second: def.meta.source,
            });
        }
        Ok(())
    }

    pub fn build(self) -> Result<Registry, RegistryError> {
        // Build secondary indices:
        // - by name
        // - by alias
        // - sort by priority, etc.
        Ok(Registry::from_builder(self)?)
    }
}
```

### Adapter Layer (temporary): ingest distributed slices into builder

For each legacy distributed slice, add an adapter function:

```rust
// crates/registry/actions/src/legacy_slice.rs
use linkme::distributed_slice;

#[distributed_slice]
pub static ACTIONS_SLICE: [ActionDef] = [..];

pub fn ingest_legacy(builder: &mut RegistryBuilder) -> Result<(), RegistryError> {
    for def in ACTIONS_SLICE {
        builder.register_action(def)?;
    }
    Ok(())
}
```

> This lets you migrate *registration* incrementally: the app can build from builder + legacy slices until everything is converted.

### Tasks

- [x] Add `XenoPlugin` trait
- [x] Add `RegistryBuilder` + `RegistryError`
- [x] Add builder collision checks:
  - [x] duplicate `meta.id`
  - [x] duplicate `meta.name` within same kind (optional but recommended)
  - [x] duplicate aliases within same kind
- [x] Add adapter modules to ingest legacy distributed slices per registry kind
- [x] Update app startup to use `RegistryBuilder` + legacy ingestion:
  - [x] `RegistryBuilder::new()`
  - [x] call `legacy_ingest_all(&mut builder)`
  - [x] `registry = builder.build()?`

### Dependencies

- none (but easiest after Phase 0 inventory)

### Risks

- *Risk:* Some registries may depend on cross-registry initialization order.
  *Mitigation:* Builder only stores defs; `build()` constructs final indices in a deterministic order.

______________________________________________________________________

# Phase 2 — Split Macros: Define-only (No Registration)

### Goals

- Keep the ergonomic macro syntax, but remove `linkme` usage from macros.

### Before → After (Action)

#### Before (legacy: define + register via slice)

```rust
use linkme::distributed_slice;
use xeno_registry_actions::ACTIONS_SLICE;

#[distributed_slice(ACTIONS_SLICE)]
pub static SAVE: ActionDef = ActionDef {
    meta: RegistryMeta {
        id: "core.save",
        name: "save",
        aliases: &["w"],
        description: "Write buffer to disk",
        priority: 0,
        source: RegistrySource::Builtin,
        required_caps: &[],
        flags: 0,
    },
    short_desc: "Save file",
    handler: save_handler,
};
```

#### After (define only)

```rust
action! {
    pub static SAVE = {
        id: "core.save",
        name: "save",
        aliases: ["w"],
        description: "Write buffer to disk",
        priority: 0,
        source: Builtin,
        required_caps: [],
        flags: 0,

        short_desc: "Save file",
        handler: save_handler,
    };
}
```

### Sketch: define-only `action!` macro

```rust
#[macro_export]
macro_rules! action {
    (
        $vis:vis static $ident:ident = {
            id: $id:expr,
            name: $name:expr,
            aliases: [$($alias:expr),* $(,)?],
            description: $desc:expr,
            priority: $prio:expr,
            source: $source:ident,
            required_caps: [$($cap:expr),* $(,)?],
            flags: $flags:expr,

            short_desc: $short:expr,
            handler: $handler:path $(,)?
        };
    ) => {
        $vis static $ident: $crate::ActionDef = $crate::ActionDef {
            meta: $crate::RegistryMeta {
                id: $id,
                name: $name,
                aliases: &[$($alias),*],
                description: $desc,
                priority: $prio,
                source: $crate::RegistrySource::$source,
                required_caps: &[$($cap),*],
                flags: $flags,
            },
            short_desc: $short,
            handler: $handler,
        };
    };
}
```

> Repeat the same pattern for `command!`, `motion!`, `gutter!`, etc.

### Tasks

- [x] Update `action!` macro to define-only (remove linkme)
- [x] Update `command!`, `motion!`, `gutter!`, etc. similarly
- [x] Add a small `source:` convention:
  - [x] Builtins should use `RegistrySource::Builtin`
  - [x] Crate-provided builtins can use `RegistrySource::Crate(env!("CARGO_PKG_NAME"))` via helper macro if desired
- [x] Ensure all defs remain `pub static` so references remain `'static`

### Dependencies

- Phase 1 builder exists (so you can register the new statics)

### Risks

- *Risk:* Macro churn touches many files.
  *Mitigation:* keep field names identical; do not change semantics; migrate one macro kind at a time.

______________________________________________________________________

# Phase 3 — Registry Migration (Explicit Registration per Crate)

### Goals

- Replace "linked crates register themselves" with a single explicit wiring graph:
  - `builtins::register_all(&mut RegistryBuilder)`

### Minimal per-crate plugin pattern

```rust
// crates/builtins/core/src/plugin.rs
pub struct CorePlugin;

impl XenoPlugin for CorePlugin {
    const ID: &'static str = "core";

    fn register(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
        reg.register_action(&crate::actions::SAVE)?;
        reg.register_action(&crate::actions::OPEN)?;
        reg.register_command(&crate::commands::WQ)?;
        Ok(())
    }
}
```

### `builtins::register_all()` with feature gating

```rust
pub fn register_all(reg: &mut RegistryBuilder) -> Result<(), RegistryError> {
    CorePlugin::register(reg)?;
    EditingPlugin::register(reg)?;

    #[cfg(feature = "lsp")]
    LspPlugin::register(reg)?;

    Ok(())
}
```

### Migration Strategy: one registry kind at a time

#### 3.1 Actions

- [x] Create `*_plugin.rs` per contributing crate (or per domain module)
- [x] Add explicit `register()` calls listing action statics
- [x] Switch app startup from legacy ingestion → `builtins::register_all`
- [x] Keep legacy ingestion only for other registry kinds still on slices

#### 3.2 Commands

- [x] Convert `command!` to define-only (if not already)
- [x] Create command registration lists per plugin
- [x] Add command collision checks for `name` and `aliases`

#### 3.3 Motions / Gutters / Handlers

- [x] Repeat conversion for each registry kind
- [x] Add any missing indices you currently computed by iterating slices

#### 3.4 Keymaps / Prefixes

- [x] If keymaps are built from distributed slices today, change to explicit registration
- [ ] Consider splitting:
  - *definitions* (available bindings)
  - *configuration* (enabled bindings per mode/profile)

### Tasks (core)

- [x] Implement `builtins::register_all()`
- [x] For each crate that currently contributes to registries:
  - [x] Add `Plugin` type implementing `XenoPlugin`
  - [x] Register all statics explicitly
- [x] Remove legacy ingestion for a registry kind once fully migrated
- [x] Add tests:
  - [x] all actions can be resolved by id
  - [x] all commands resolve by name/alias
  - [x] no duplicate aliases

### Dependencies

- Phase 2 macros (define-only)
- Phase 1 builder + adapters (to keep partial migration working)

### Risks

- *Risk:* Missed registration leads to "feature disappeared."
  *Mitigation:* temporary parity test: compare counts/IDs between legacy slice ingestion and explicit registration for the same kind until complete.

______________________________________________________________________

# Phase 4 — Remove `linkme` and Distributed Slices

### Goals

- Delete the pattern entirely once explicit registration has parity.

### Tasks

- [x] Remove all `#[distributed_slice]` declarations and slice globals
- [x] Delete legacy adapter ingestion modules
- [x] Remove `linkme` dependency from all crates
- [x] Simplify macros: eliminate linkme-related helpers

### Dependencies

- Phase 3 complete for all registry kinds

### Risks

- *Risk:* Hidden "side registry" exists (not in plan) still using slices.
  *Mitigation:* repo-wide search gate (`distributed_slice`, `linkme::`).

______________________________________________________________________

# Phase 5 — High-Priority LSP Refactors (Performance + Correctness)

This phase is independent and can run in parallel after Phase 0.

## 5A — Remove Unconditional Content Cloning

### Goal

- Avoid cloning full document text unless a full sync is required.

### Before (pattern: always clone)

```rust
let content = doc.to_string(); // unconditional
match try_incremental(...) {
    Ok(changes) => send_incremental(changes),
    Err(_) => send_full(content),
}
```

### After (lazy snapshot provider)

```rust
let mut snapshot: Option<String> = None;
let mut get_snapshot = || -> &str {
    snapshot.get_or_insert_with(|| doc.to_string()).as_str()
};

match try_incremental(...) {
    Ok(changes) => send_incremental(changes),
    Err(_) => send_full(get_snapshot()),
}
```

### Tasks

- [x] Refactor immediate flush path to delay `to_string()`
- [x] Refactor debounced flush path to delay `to_string()`
- [x] Add counters:
  - [x] #full_sync
  - [x] #incremental
  - [x] bytes snapshotted per second/tick
- [x] Add perf regression test (basic):
  - [x] editing small ranges does not snapshot full doc repeatedly

### Risks

- *Risk:* Borrowing/lifetimes get tricky if snapshot must outlive closure.
  *Mitigation:* snapshot stored in local `Option<String>` owned by the flush function.

______________________________________________________________________

## 5B — Debounce Discipline (Send Once After Quiet Period)

### Goal

- Accumulate changes per doc, flush a single notification after quiet period.

### Tasks

- [x] Ensure `PendingLspState` is per-document with:
  - [x] pending incremental changes queue (or coalesced)
  - [x] last_change_time
  - [x] editor_version at last change
  - [x] force_full_sync flag
- [x] When changes arrive:
  - [x] update state, do **not** send immediately unless policy says so
- [x] On tick:
  - [x] flush only docs whose quiet period has elapsed
  - [x] flush at most N docs per tick (budget)

### Risks

- *Risk:* "Never flush" bug if timers are wrong.
  *Mitigation:* add debug log when state exceeds max age without flush; add unit test with fake clock.

______________________________________________________________________

## 5C — Version Discipline + Recovery

### Goal

- Treat version mismatch as first-class; recover by forced full sync.

### Tasks

- [x] Make `DocumentStateManager` the authority for:
  - [x] current server version per doc
  - [x] pending requests (didOpen/didChange) and expected acks
- [x] On mismatch (server says old version, or rejects change):
  - [x] set `force_full_sync`
  - [x] clear incremental queue
  - [x] send full snapshot with new version
- [x] On large edit (undo/redo or bulk):
  - [x] explicitly call `force_full_sync()` and bump version
- [x] Add tests:
  - [x] incremental sequence produces monotonic versions
  - [x] simulated mismatch triggers full sync and resets state

### Risks

- *Risk:* Over-eager full sync harms perf.
  *Mitigation:* count and log reasons for full sync; tune thresholds.

______________________________________________________________________

# Phase 6 — Architectural Improvements for Scale (Budgets + Observability + Determinism)

## 6A — Budgeted Async Drain in Tick/Hook Runtime

### Goals

- Prevent a single tick from being dominated by hook or async drains.

### Tasks

- [ ] Introduce `drain_budget()` pattern:
  - [ ] process at most N completions per tick
  - [ ] time-budget option (e.g. 1–2 ms)
- [ ] Apply budget to:
  - [ ] hook runtime futures (if any)
  - [ ] LSP main loop `FuturesUnordered` drain
- [ ] Add debug counters: completions processed per tick, backlog size

### Risks

- *Risk:* Starvation if budget too low.
  *Mitigation:* dynamic budget when backlog is large; ensure some minimum forward progress.

______________________________________________________________________

## 6B — Registry Diagnostics and Developer UX

Even after migrating away from distributed slices, registry scale can still bite. Make it inspectable.

### Tasks

- [ ] Add `:registry` command:
  - [ ] list items by kind, sorted by priority
  - [ ] filter by prefix (`core.*`, `lsp.*`)
  - [ ] show `meta.source`, required caps, flags, description
- [ ] Print startup summary:
  - [ ] counts per kind
  - [ ] collisions as hard errors (not warnings)

### Risks

- *Risk:* Spending time on tooling instead of product.
  *Mitigation:* keep it minimal; prioritize collision error messages first.

______________________________________________________________________

## 6C — Effect System Guardrails (Optional but valuable)

### Tasks

- [ ] Add dev-mode assertion: "unhandled effect variants" should not silently log
- [ ] Add tracing spans around effect application:
  - [ ] effect kind
  - [ ] duration
  - [ ] whether it triggered LSP sync

### Risks

- *Risk:* Too noisy tracing.
  *Mitigation:* sample or gate behind `debug_assertions` / feature flag.

______________________________________________________________________

# Suggested Execution Order (Practical)

1. **Phase 0** (baseline)
2. **Phase 5A** (remove unconditional LSP clones) — quick win, localized risk
3. **Phase 1** (builder + adapters)
4. **Phase 2** (define-only macros)
5. **Phase 3** migrate registries incrementally:
   - actions → commands → motions → gutters → handlers → keymaps
6. **Phase 4** remove linkme
7. **Phase 5B/5C** (debounce + version discipline)
8. **Phase 6** budgets + observability

______________________________________________________________________

# Progress Tracker (Copy/Paste)

## Phase 0

- [x] Inventory report
- [x] Collision audit
- [x] Smoke test checklist
- [x] CI lint checks
- [x] LSP clone counters

## Phase 1

- [x] `XenoPlugin` trait
- [x] `RegistryBuilder` + errors
- [x] Collision validation (id/name/alias)
- [x] Legacy ingestion adapters
- [x] App startup builds registry from builder

## Phase 2

- [x] `action!` define-only
- [x] `command!` define-only
- [x] `motion!` define-only
- [x] `gutter!` define-only
- [x] All registry macros compile without `linkme`

## Phase 3

- [x] `builtins::register_all()`
- [x] Actions migrated
- [x] Commands migrated
- [x] Motions migrated
- [x] Gutters migrated
- [x] Handlers migrated
- [x] Keymaps migrated
- [x] Parity tests (legacy vs explicit) removed after migration

## Phase 4

- [x] Delete distributed slices
- [x] Remove `linkme` deps

## Phase 5

- [x] Lazy snapshot provider everywhere
- [x] Debounce discipline
- [x] Version reconciliation + recovery
- [x] Tests for version/mismatch recovery

## Phase 6

- [ ] Budgeted drain patterns
- [ ] `:registry` command
- [ ] Tracing spans and counters

______________________________________________________________________

*Plan generated with assistance from ChatGPT based on xeno codemap analysis.*
