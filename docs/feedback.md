## 1) “Orthogonal” + `linkme`: keep the convenience, but add explicit *boundaries*

Your goal says “no tight coupling… event emitter/receiver… heavily utilize `linkme` distributed_slices” . `linkme` does give the “drop a file in and it’s registered” feel , but it also creates *implicit dependencies* that can become the opposite of orthogonal:

- **Hidden initialization ordering**: anything that “collects all slices” becomes a de-facto global init point.
- **Name collisions**: two actions/commands/hooks with the same name won’t fail until runtime unless you enforce it.
- **Discoverability**: “who registers what?” becomes grep-only.

**Concrete improvement:** keep distributed slices *only* as a collection mechanism, but force everything into an explicit “registry build” step that:

- validates uniqueness (`ActionId`, command names, hook names),
- produces an immutable `Registry` object passed around (no global map lookups sprinkled everywhere).

This preserves “drop file in” ergonomics, while making module boundaries explicit at the registry layer.

## 2) Kill stringly-typed actions at the core boundary (keep strings at the edges)

Right now keystrokes ultimately resolve to `KeyResult::Action { name: ... }` style dispatch (you can see the string name usage in several paths) . This is a classic long-term pain point in editors:

- you lose refactorability (rename breaks at runtime),
- you can’t easily prove “all actions are registered”,
- extensions/higher layers end up relying on ad-hoc strings too.

**Concrete improvement:** introduce a typed action identifier:

- `ActionId(u32)` or interned symbol (`SmolStr` → intern table at registry build time),
- keep “human names” only for help/docs/user config,
- let InputHandler output `ActionId`, not `&'static str`.

Then use proc macros to generate:

- the `ActionDescriptor { id, name, required_caps, fn_ptr, … }`
- optional default keybindings per mode.

This aligns perfectly with your “heavy proc macro usage” goal .

## 3) InputHandler: reduce cloning + make “params” (count/register/extend) a first-class parse phase

Two concrete issues to tighten:

### 3a) Command mode cloning

`handle_command_key` takes `mut input: String` and then writes it back into the mode on every keystroke . That’s fine for small strings, but it’s unnecessary churn.

**Concrete improvement:** store the input in `self.mode` and mutate it in place. i.e. the handler should borrow/match `Mode::Command { .. }` and edit the internal string, rather than threading it through function args.

### 3b) “count/register/extend” lifecycle

You’re already tracking `count`, `register`, `extend` and resetting them . But you currently have a lot of duplicated “compute count/extend/register → reset_params → return Action” logic .

**Concrete improvement:** turn it into a pipeline:

1. **Parse params** (count/register/extend) from a stream of keys
1. **Resolve binding** (mode → action)
1. **Emit dispatch** (ActionId + parsed params)

That makes multi-key chords and “pending action” cases much cleaner too.

### 3c) Shift/extend semantics are clever but a bit magical

`extend_and_lower_if_shift()` implicitly toggles extend and modifies lookup keys . This works, but it creates surprising interactions over time (especially once users configure bindings heavily).

**Concrete improvement:** encode “extend” as either:

- an explicit modifier concept in the binding system, or
- a separate “selection-extend mode” state (like Kakoune-ish semantics) rather than “Shift always means extend unless bound”.

At minimum, I’d add tests covering “uppercase has its own binding vs falls back to lowercase” .

## 4) Selection normalization: fix primary stability for duplicates + make normalization cheaper when building

Your selection model is solid and has nice utilities (normalize vs merge-adjacent) . The weak spot is **primary preservation** when duplicates exist.

Example: `from_vec()` preserves primary by equality search after moving into SmallVec . If two ranges are equal, `.position()` will pick the first, which may not be the original primary.

**Concrete improvements:**

- Track primary by **original index**, not equality. During normalization/merge, carry `(range, was_primary, orig_index)` and compute the new primary deterministically.

- Add **property tests**:

  - normalization is idempotent
  - primary is stable under sorting/merging when ranges are duplicated
  - merging logic chooses expected primary when primary is contained by a merged span

Also: `push()` and `replace()` normalize every time . That’s fine for “few cursors”, but if you ever build large selections (search results, multi-cursor from file), a builder API that normalizes once can be a big win.

## 5) ChangeSet/Transaction: correct, but performance will crater on large inserts

You’re using character counts (`chars().count()`) in hot paths:

- insert updates len_after via `text.chars().count()`
- apply increments `pos` the same way
- map_pos recomputes insert lengths repeatedly
- compose repeatedly does `t.chars().take(..).collect()` and re-counts

That’s all O(n) scanning per operation, and compose can amplify it badly.

**Concrete improvements:**

- Store `Insert { text: Tendril, char_len: CharLen }` so counting is done once.

- When composing/splitting inserts, avoid `.chars().take()` from the front repeatedly. Prefer slicing by cached boundaries (or keep inserts small/normalized so you don’t split often).

- Add tests for invariants:

  - `apply(invert(doc))` roundtrips the document
  - `compose(a,b)` equals applying `a` then `b` (property test on random ops)

## 6) Testing: you already picked the right battleground — now add fuzz/property tests

Your kitty integration harness is a huge advantage for catching real drift .

To complement that:

- **Property tests** for Selection + ChangeSet invariants (this will find edge cases faster than any hand-written suite).
- **Golden/snapshot tests** for rendering: stable text fixtures → expected screen output.
- If you keep the “orthogonal event emitter/receiver” goal, add tests that ensure emitters don’t import receiver crates (compile-time dependency tests).

## A practical “next 5 commits” roadmap

If I were sequencing this for max leverage:

1. Introduce `ActionId` and convert keybinding dispatch off strings.
1. Add action metadata (required capabilities) and registry validation.
1. Cache char lengths in ChangeSet inserts; reduce `.chars().count()` churn.
1. Fix Selection primary stability for duplicates + add property tests.
1. Expand kitty integration harness coverage.
