# Syntax Highlighting & Tree-sitter Architecture

Scope: syntax highlighting + incremental parsing + grammar loading in Xeno. Tree-sitter via tree-house. Orchestration in editor-side SyntaxManager. Runtime assets and loaders in xeno-runtime-language.

## 0. Goals / invariants

Hard invariants

* UI thread never blocks on parsing.
* single-flight per DocumentId: max 1 parse task running per doc.
* bounded global concurrency: max N parse tasks running across docs (semaphore).
* install discipline: parse result installs only if parse_version == doc.current_version at install time.
* no abort storms: edits do not abort running spawn_blocking parses; edits set dirty and schedule follow-up.
* cooldown after failures/timeouts: prevent retry loops.

Memory invariants

* tier-based tree retention (drop when hidden per tier policy).
* injection gating for large tiers to prevent layer explosion.
* sync incremental updates only for Tier S.

## 1. Module ownership

xeno-runtime-language

* LanguageDb: language registry (from languages.kdl) with metadata + query assets + grammar name.
* LanguageLoader: resolves grammar dylibs and query assets; provides injection gating view.
* Syntax: tree-house wrapper; provides new/new_with_options and update/update_with_options; supports InjectionPolicy and parse_timeout.

xeno-editor

* SyntaxManager: scheduler state machine; tier policy; semaphores; debounce/cooldown/retention; kicks background parses and installs results.

## 2. Language registry (LanguageDb)

Input source

* languages.kdl defines language entries.

Per-language fields

* metadata: name, extensions, filenames, globs, shebang patterns
* queries: highlights.scm, injections.scm, locals.scm etc (paths or embedded assets)
* grammar: tree-sitter grammar id/name used for dylib lookup

Runtime expectations

* LanguageDb can resolve a LanguageId from file path/shebang/etc.
* LanguageDb provides query sources for tree-house.

## 3. Tiered syntax policy

Tier selection

* size = Rope::len_bytes()

Tiers

* S: size <= 256 KiB
* M: size <= 1 MiB
* L: size > 1 MiB

Tier behavior matrix (baseline)

* S

  * parse_timeout: 500ms
  * debounce: ~80ms
  * cooldown: short (timeout ~400ms, error ~150ms)
  * injections: eager
  * retention when hidden: keep
  * sync incremental updates allowed
* M

  * parse_timeout: ~1.2s
  * debounce: ~140ms
  * cooldown: timeout ~2s, error ~250ms
  * injections: eager
  * retention when hidden: drop after TTL (60s)
  * background-only updates (no sync incremental)
* L

  * parse_timeout: ~3s
  * debounce: ~250ms
  * cooldown: timeout ~10s, error ~2s
  * injections: disabled
  * retention when hidden: drop immediately
  * background-only updates (no sync incremental)

Policy is pure function:

* inputs: bytes, hotness (Visible/Warm/Cold)
* output: TierCfg (timeouts, debounce, cooldowns, injection policy, retention policy, parse_when_hidden)

Hotness semantics

* Visible: doc currently rendered; parse allowed; retention keep.
* Warm: not rendered but likely soon; retention treated like Visible (extend last_visible_at), parsing typically not allowed unless explicitly configured.
* Cold: hidden; retention applies; parsing typically disallowed.

## 4. SyntaxManager scheduler

Inputs

* note_edit(doc_id): records last_edit_at
* ensure_syntax(doc_id, doc_version, language_id, content, current_syntax, syntax_dirty, hotness, loader): polling entrypoint called from render loop or document pipeline

Per-doc state

* last_edit_at: Instant
* last_visible_at: Instant
* cooldown_until: Option<Instant>
* inflight: Option<PendingSyntaxTask { doc_version, tier, started_at, JoinHandle<Result<Syntax, SyntaxError>> }>

Global state

* permits: tokio::Semaphore (default 2); use try_acquire_owned for non-blocking scheduler behavior

Algorithm (ensure_syntax)

1. early outs

   * if language_id None => NoLanguage
   * compute bytes, tier, cfg
2. update visibility

   * if hotness is Visible or Warm => update last_visible_at = now
3. apply retention

   * cfg.retention_hidden:

     * Keep: no-op
     * DropAfter(TTL): if hidden/cold and now-last_visible_at > TTL => drop syntax, mark dirty
     * DropWhenHidden: if hidden/cold => drop syntax, mark dirty
4. if syntax exists and not dirty => Ready
5. hidden parsing gating

   * if hotness != Visible and cfg.parse_when_hidden == false => Disabled (do not schedule)
6. poll inflight

   * if inflight exists and not finished => Pending
   * if finished => join via now_or_never (no executor); handle result:

     * Ok(syntax) and version matches => install, clear dirty, clear cooldown, Ready
     * Ok(syntax) stale => discard; keep dirty
     * Err(Timeout) => set cooldown_until = now + cfg.cooldown_on_timeout; CoolingDown
     * Err(other) => set cooldown_until = now + cfg.cooldown_on_error; CoolingDown
     * JoinError => set cooldown_until similarly; CoolingDown
7. debounce

   * if now - last_edit_at < cfg.debounce => Pending
8. cooldown

   * if cooldown_until present and now < cooldown_until => CoolingDown
9. concurrency cap

   * try_acquire_owned permit; if none => Throttled
10. spawn parse

* build SyntaxOptions { parse_timeout: cfg.parse_timeout, injections: cfg.injections }
* spawn_blocking: hold permit in closure; call Syntax::new_with_options(content.slice(..), lang_id, loader, opts)
* store inflight task; return Kicked

No abort/restart on edits

* edits during inflight do not abort the inflight task
* caller is expected to keep syntax_dirty=true so a new parse is attempted after inflight completes and debounce passes

## 5. Runtime Syntax API

Types

* InjectionPolicy: Eager | Disabled
* SyntaxOptions { parse_timeout: Duration, injections: InjectionPolicy }

Constructor / update

* Syntax::new(...) uses SyntaxOptions::default
* Syntax::new_with_options(source, language, loader, opts)

  * uses loader.with_injections(opts.injections == Eager)
  * passes opts.parse_timeout into tree-house Syntax constructor
* Syntax::update_with_options(...) same gating and timeout
* incremental updates from ChangeSet go through generate_edits and then update_with_options

Error taxonomy

* SyntaxError must include Timeout to support cooldown path
* map tree-house timeout errors into SyntaxError::Timeout

## 6. Language injections

Mechanism

* tree-house multi-layer syntax tree from injections.scm and injected language resolution

Gating

* implement LanguageLoader::with_injections(bool) returning a view wrapper:

  * when disabled: injections_query returns None and injected language resolution returns None
  * other methods delegate to base loader unchanged

Policy binding

* Tier L sets injections Disabled
* Tier S/M default Eager

Optional future extension

* Lazy injections: build injection layers only for viewport range with LRU cap; requires tree-house support and viewport plumbing

## 7. Grammar loading and compilation

Bundle-first load order

1. bundled: $XENO_RUNTIME/language/grammars
2. installed: <exe>/../share/xeno/grammars
3. dev: workspace target/grammars
4. cache: ~/.cache/xeno/grammars
5. helix runtime dirs

JIT fallback

* only if grammar not found in above search paths
* JIT can be disabled via env var:

  * XENO_DISABLE_JIT_GRAMMARS=1 => return GrammarError::JitDisabled
* build failures must be surfaced distinctly:

  * GrammarError::BuildFailed (not NotFound)

Build mechanics

* fetch grammar repo (pinned rev preferred)
* compile via cc crate into shared library in cache/build dir
* load produced dylib via libloading

## 8. Incremental updates (ChangeSet -> InputEdit)

* convert ChangeSet ops into Vec<InputEdit>
* must compute correct byte offsets and Points

  * start_byte/old_end_byte/new_end_byte computed via rope char_to_byte
  * start_point/old_end_point from rope line/byte mapping
  * new_end_point computed from inserted text byte structure (newline accounting)
* point.column is byte-based, not char count

Gating

* sync incremental update on commit path only for Tier S (<=256KiB)
* Tier M/L: always mark syntax_dirty and rely on SyntaxManager background parse
* this prevents large docs from reintroducing UI jitter via clone+update work

## 9. Operational checklist (agent)

Search/entrypoints

* grep "ensure_syntax(" callsites: ensure hotness passed correctly (Visible for rendered docs, Warm for MRU, Cold otherwise)
* grep "update_from_changeset" in Document: ensure byte-size gate exists
* grep "with_injections" in runtime loader: ensure injection gating wrapper is used by Syntax::new_with_options/update_with_options
* grep "XENO_DISABLE_JIT_GRAMMARS" in grammar loader: ensure JIT disable returns distinct error

Tests

* tier selection boundaries
* single-flight: repeated edits do not spawn >1 inflight per doc
* global cap: N docs dirty => inflight count <= semaphore limit
* debounce: no spawn before debounce window
* cooldown: timeout triggers cooldown; no immediate respawn; respawn after cooldown
* retention: M drops after TTL hidden; L drops immediately hidden; S keeps
* InputEdit points: incremental update preserves stable highlights across multiline insert/delete

Done criteria

* cargo check + full tests pass
* open 2MB file: no UI stalls, retries occur after idle, injections disabled
* run without system compiler + missing grammar bundle: clean JitDisabled error (no crash, no loop)

