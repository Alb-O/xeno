# Xeno Registry System (Agent Guide)

This is an agent-facing, task-first map of Xeno’s registry system: how items are defined, registered, indexed, extended (plugins + runtime overlays), and finally invoked with capability gating.

Scope: actions, commands, motions, text objects, options, themes, gutters, statusline, hooks, notifications.

---

## Mental model

- A “registry item” is a `&'static T` where `T: RegistryEntry` (has a `meta: RegistryMeta`).
- Compile-time registration uses `inventory` (`Reg<T>` / `RegSlice<T>`). Startup builds a `RegistryDb` by iterating inventory and building per-domain indices.
- Indices are strict about **ID invariants**. Name/alias collisions are allowed (with deterministic winner selection).
- Runtime extensibility is via `RuntimeRegistry<T>`: a thread-safe overlay on top of the built index. Lookup is layered (extras-first) and atomic registration is supported.
- Dispatch (`Editor::run_invocation`) looks up the action/command by registry name/alias/id, then gates execution via `required_caps` from metadata.

---

## Where the “source of truth” lives

### Core (xeno-registry-core)
- `RegistryMeta`, `RegistrySource`, `Capability`
- `RegistryEntry`, `RegistryMetadata`, `impl_registry_entry!`
- `RegistryBuilder<T>`: build-time invariant enforcement, collision tracking
- `RegistryIndex<T>`: built index, ID-first lookup
- `RuntimeRegistry<T>`: runtime overlay + atomic batch insertion
- `DuplicatePolicy`, `Collision`, insertion error types

### Registry crate (xeno-registry)
- `inventory::{Reg<T>, RegSlice<T>}` wrappers
- Domain modules: `actions`, `motions`, `commands`, …
- `db::{RegistryDb, RegistryDbBuilder, RegistryIndices, plugin::{XenoPlugin, register_plugin!}}`

### Editor
- `Editor::run_invocation` / `invoke_action` / `invoke_command`
- Capability gating via `EditorContext::check_all_capabilities(required_caps)`
- Hook emission around invocations

---

## Hard Rules (do not violate)

| Rule | Why it exists | How to comply (agent checklist) | Failure mode |
|---|---|---|---|
| IDs are sacred (global uniqueness per domain) | IDs are the stable identity and the first lookup namespace | Use macro-generated IDs (`concat!(env!("CARGO_PKG_NAME"), "::", name)`); don’t handwrite IDs unless you know you need stability across crates/plugins | Build-time fatal: duplicate IDs |
| “ID shadowing” is forbidden in the base index | If name/alias insertion could shadow an existing ID, `get(id)` would become ambiguous | Never set `name`/`alias` equal to some other item’s `id`; avoid “looks like an ID” strings | Build-time fatal: “ID shadowing” |
| Name/alias collisions are allowed but deterministic | UX keys (name/alias) are flexible, but must be predictable | If you intentionally collide, set `priority` and `source` intentionally; otherwise keep names unique | Surprise winner changes if priorities change |
| Prefer `id` for machine references, `name` for humans | IDs are stable; names are UI-facing | Code should store/emit IDs; UI should display `name` + `description` | Breakage when names change |
| `required_caps` must match runtime behavior | Dispatch gates execution based on `required_caps` | Add caps in the macro/def; keep minimal; validate via invocations in tests | CapabilityDenied, readonly denial, or unsafe behavior |
| Registry items must be leak-to-static (current policy) | Lookups store `&'static T` and typed handles hold `&'static T` | `static DEF: T = ...` OR `Box::leak(Box::new(T{...}))` (plugins/extensions) | Lifetime mismatch / cannot store |

---

## Registration paths

### 3.1 Builtins (inventory)
Use domain macros (`action!`, `motion!`, `command!`, …). These:
- Define a `static <DOMAIN>_<name>: <DomainDef>`
- Define a typed handle `pub const <name>: Key<DomainDef>`
- `inventory::submit! { Reg(&static_def) }`
- Some domains also submit `RegSlice` side tables (keybindings, prefixes, etc.)

### 3.2 Startup plugins (inventory-driven, builder extension)
Implement `XenoPlugin` and register with `register_plugin!(YourPlugin)`.
- Plugin is discovered via `inventory` and executed during `RegistryDb` construction.
- Plugin registers new items by pushing definitions into `RegistryDbBuilder` fields.

### 3.3 Runtime overlays (after startup)
Use `RuntimeRegistry<T>::register` / `register_many` (atomic batch). Current policy accepts leak-to-static entries.

---

## Lookup semantics (what “get(name)” really means)

- ID-first: `RegistryIndex::get` checks the ID map first, then name, then aliases.
- RuntimeRegistry layering: for each namespace (ID, name, alias), “extras first then builtins”.
- Collisions: build-time collisions can be inspected (per index) to expose diagnostics.

Consequence: if you want an override behavior for UI keys, colliding on *name* is viable (priority decides winner). If you want a true identity override, you need an **explicit ID override mechanism** (see "Override semantics").

---

## Task-first recipes

### Recipe A: Add a new builtin action

Goal: define a new action `foo_bar` with keybindings.

Edits (typical):
1. `crates/registry/src/actions/builtins/<some_module>.rs`
2. `crates/registry/src/actions/builtins/mod.rs` (ensure module is included)
3. (Optional) `crates/editor/...` if the handler needs new editor APIs

Steps:
- Add an action using the macro:

```rust
action!(foo_bar, {
    description: "Do foo then bar",
    bindings: r#"normal "g b" "foo_bar""#,
    // optional:
    // aliases: &["fb"],
    // short_desc: "FooBar",
    // priority: 10,
    // caps: &[Capability::Edit],
    // flags: actions::flags::NONE,
}, |ctx| {
    // handler body -> ActionResult
    ActionResult::Noop
});
```

Verification:
- `rg -n "foo_bar" crates/registry/src/actions`
- `cargo test -p xeno-registry` (or workspace equivalent)
- Manual: bind key, invoke, ensure capability behavior matches.

Gotchas:
- If the action emits motions, prefer stable motion ID constants rather than strings.
- If you require editing but forget `caps`, readonly buffers may still execute (or later be denied in unexpected places).

---

### Recipe B: Add a new builtin motion

Edits:
1. `crates/registry/src/motions/builtins/<some_module>.rs`
2. `crates/registry/src/motions/builtins/mod.rs`

Example:

```rust
motion!(my_motion, {
    description: "My cursor movement",
    // caps: &[Capability::Edit], // if needed
}, |text, range, count, extend| {
    range // compute new Range
});
```

Verification:
- `cargo test -p xeno-registry`
- Add an action that references this motion (or expose it in `motions::keys`) and manually exercise.

---

### Recipe C: Add a startup plugin that registers registry items

Goal: ship a crate that adds actions/motions/etc during `RegistryDb` build.

Edits (in plugin crate):
1. Implement the plugin:

```rust
use xeno_registry::db::builder::{RegistryDbBuilder, RegistryError};
use xeno_registry::db::plugin::XenoPlugin;

pub struct MyPlugin;

impl XenoPlugin for MyPlugin {
    const ID: &'static str = "my_plugin";
    fn register(db: &mut RegistryDbBuilder) -> Result<(), RegistryError> {
        // definitions must be &'static
        db.register_action(Box::leak(Box::new(make_action_def())));
        Ok(())
    }
}
```

2. Register it:

```rust
xeno_registry::register_plugin!(MyPlugin);
```

Verification:
- `rg -n "register_plugin!" -S crates`
- `cargo test -p xeno-registry --features db`
- Runtime smoke: ensure plugin errors are not swallowed silently (check logs).

Gotchas:
- Plugin `ID` should be globally unique for diagnostics.
- Avoid side effects in plugin `register`; keep it pure “push definitions into builder”.

---

### Recipe D: Register many runtime entries atomically (load-only extension)

Goal: install a batch of actions in one shot (all-or-nothing).

Pseudo-flow:
1. Parse extension config.
2. Build `Vec<&'static ActionDef>` via `Box::leak`.
3. Call `ACTIONS.register_many(&defs)` (or similar).

Verification:
- Unit test: ensure partial failure leaves registry unchanged (atomicity).
- Regression test: ensure collisions follow expected `DuplicatePolicy`.

Gotchas:
- If actions are required to participate in `ActionId` numeric mapping, runtime registration will **not** update that mapping unless you also update/replace the mapping. Prefer startup registration for ActionId-stable items.

---

## Capability enforcement & dispatch (how metadata becomes behavior)

Key linkage:
- `RegistryMeta.required_caps: &'static [Capability]` is *the* declarative contract for an item.
- Domain defs expose `required_caps()` by delegating to `meta.required_caps`.

Dispatch enforcement:
- Invocation path resolves item via registry lookup (name/alias/id).
- The editor checks capabilities (typically `EditorContext::check_all_capabilities(required_caps)`).
- If enforcement is enabled, missing caps block the invocation (and return a capability-denied result).
- Some policies also gate readonly execution if `required_caps` imply mutation.

Agent rule of thumb:
- If the handler can mutate document state, add the edit capability. If it reads-only (navigation, inspection), keep caps empty.
- For new capabilities, update:
  - the `Capability` enum
  - editor capability-check implementation
  - docs + tests (capability denied path)

---

## Override semantics (names vs IDs)

### Name/alias override (safe and already supported)
If you want to “replace” behavior under a UI name, define a new item with the same `name` or `alias` and set a higher `priority`. Deterministic selection chooses the winner.

This does **not** change identity: `get(old_id)` still resolves to the old item.

### ID override (explicit, opt-in)
If you want identity override (`get(id)` resolves to the new entry), use `RuntimeRegistry::set_allow_id_overrides(true)`.
When enabled:
- Runtime definitions can shadow built-in IDs.
- Winner is selected by `DuplicatePolicy` (typically priority-based).
- Collisions are recorded with `KeyKind::Id`.
- Name/alias shadowing of the ID remains forbidden for unrelated definitions.

---

## Static lifetime today; Arc-based entries tomorrow

Current state: registries store `&'static T` and typed handles are `Key<T> = wrapper(&'static T)`. This is a deliberate choice for zero-cost lookups and trivial sharing.

If you move to `Arc<T>` entries, you’ll need to decide what becomes the “typed handle”:

### Option 1: `Key<T> = Arc<T>` (cheap clone, simplest)
Pros: easy; no unsafe; no leaking; plugins can unload if you keep Arc counts.
Cons: breaks the “zero-sized key” property; changes APIs everywhere.

### Option 2: Keep `Key<T>` but store an index + generation
Pros: handles remain Copy-sized; underlying storage can be `Vec<Arc<T>>`.
Cons: needs indirection; must solve ABA / removal.

Given Xeno’s current “load-only extensions” policy, the cleanest future state is usually:
- `RegistryIndex<T>` stores `Arc<T>` internally.
- `Key<T>` becomes a thin `Arc<T>` newtype.

---

## Grep/verify cheatsheet

- Find all registry macros:
  - `rg -n "macro_rules! (action|motion|command|text_object)" crates/registry`
- Find all plugin registrations:
  - `rg -n "register_plugin!" crates`
- Find capability gating:
  - `rg -n "check_all_capabilities|required_caps|CapabilityDenied" crates/editor`
- Inspect collisions/overrides:
  - `rg -n "collisions\\(|Collision" crates/registry/core`

---

## Minimal test checklist

- [ ] Build-time: duplicate IDs => fatal
- [ ] Build-time: name/alias collision => deterministic winner (policy)
- [ ] Build-time: “ID shadowing” (name/alias equals someone else’s id) => fatal
- [ ] Runtime: register_many is atomic (failure leaves state unchanged)
- [ ] Runtime: explicit ID override shadows the old entry (and is recorded)
- [ ] Dispatch: missing caps => CapabilityDenied when enforcement enabled
- [ ] Dispatch: log-only mode continues execution but warns
