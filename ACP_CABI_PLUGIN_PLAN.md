# Tome term-only plugin system via C-ABI (cabi)

This document plans a refactor of the current built-in ACP integration (in `crates/tome-term/src/acp/`) into a **runtime-loadable C-ABI plugin**, while also defining a **term-only plugin API** that supports:

- Namespaced runtime commands (e.g. `acp.start`, `acp.toggle`)
- Host-rendered plugin UI surfaces (panels) suitable for chat/transcript flows
- Background async work (Tokio, subprocesses) with safe UI-thread application
- Auto-loading plugins from standard directories at startup

It is written to be implementation-ready: it includes interface sketches, task breakdown, and file-level integration points.

______________________________________________________________________

## 0) Context / current state

### What exists today

- **C-ABI plugin loader (minimal)**

  - Loader: `crates/tome-core/src/ext/plugins/cabi.rs`
  - Types: `crates/tome-cabi-types/src/lib.rs`
  - Demo plugin: `crates/demo-cabi-plugin/src/lib.rs`
  - Current host table is **tiny**: `TomeHostV1 { abi_version, log }`

- **ACP integration (built-in to `tome-term`)**

  - Runtime / subprocess: `crates/tome-term/src/acp/client.rs`
  - UI model: `crates/tome-term/src/acp/types.rs`
  - UI rendering: `crates/tome-term/src/render/agent.rs`
  - Editor integration: `crates/tome-term/src/editor/mod.rs` (`Editor::do_agent_*`, `poll_agent_events`)

- **Command system split**

  - Compile-time commands in `tome-core` via `linkme`: `crates/tome-core/src/ext/mod.rs` and `crates/tome-core/src/ext/commands/*`
  - Runtime commands do not exist yet.

### Constraints

- **Term-only**: plugin API can assume `tome-term` as the host (no need to support other frontends).
- **Dynamic library boundary**: no Rust trait objects or `ratatui` types across ABI.
- **UI is synchronous**: anything async (ACP, terminal tools, subprocess IO) must be mediated into UI-thread mutations.

______________________________________________________________________

## 1) Goals / non-goals

### Goals

1. ACP becomes a **plugin**: `opencode acp` orchestration runs in a `cdylib`.
1. The plugin registers **namespaced commands**: `acp.toggle`, `acp.start`, `acp.stop`, `acp.insert_last`, …
1. The plugin can drive a **chat-like panel** (transcript + multiline input), rendered by the host.
1. Plugins can request **interactive permissions** and receive decisions.
1. Tome **auto-loads plugins** from standard directories at startup.

### Non-goals (initially)

- Fully general UI rendering from plugins (host will render generic panels).
- A stable cross-frontend API (we explicitly target `tome-term`).
- A sandbox/security model beyond explicit permission prompts + path restrictions.

______________________________________________________________________

## 2) Architecture overview

### High-level idea

- The plugin runs ACP (Tokio + subprocess) and emits **events** into a queue.
- The host (UI thread) **polls** each plugin for events each tick and applies them:
  - append transcript lines
  - open/close/focus panels
  - request permission UI
  - show status/messages

This matches the existing Tome design constraint: **all `Editor` mutations stay on the UI thread**.

### Key decision: host-rendered panels

Plugins do not draw. Instead the host provides a small “panel surface” abstraction:

- panel lifecycle: create, open/close, focus
- content updates: append transcript items, set status
- input: host collects keystrokes; on submit it calls into the plugin

This keeps the ABI stable and avoids binding plugins to `ratatui` types.

______________________________________________________________________

## 3) New plugin API: `tome-cabi-types` v2

### 3.1 Versioning strategy

Do **not** mutate `TomeHostV1` / `TomeGuestV1` in-place.

Instead add a v2 entry symbol and v2 tables:

- v1 symbol (existing): `tome_plugin_entry`
- v2 symbol (new): `tome_plugin_entry_v2`

Host load strategy:

1. Try `tome_plugin_entry_v2`
1. If missing, fall back to `tome_plugin_entry` (v1)

This avoids ABI ambiguity and keeps existing plugins working.

### 3.2 FFI-safe primitive types

Add these to `crates/tome-cabi-types/src/lib.rs`:

```rust
#[repr(C)]
pub struct TomeStr {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
pub struct TomeStrArray {
    pub ptr: *const TomeStr,
    pub len: usize,
}

#[repr(C)]
pub struct TomeBool(pub u8); // 0/1
```

### 3.3 Guest-owned strings for events

Events often carry text. The simplest safe approach is guest-owned allocations + explicit free:

```rust
#[repr(C)]
pub struct TomeOwnedStr {
    pub ptr: *mut u8,
    pub len: usize,
}

pub type TomeFreeStrFn = extern "C" fn(s: TomeOwnedStr);
```

Contract:

- Plugin returns `TomeOwnedStr` in events.
- Host copies into its own `String`.
- Host calls `guest.free_str(...)` to release plugin allocation.

This avoids relying on “borrowed pointers valid until next call”.

### 3.4 Namespaced commands

Commands are runtime-registered. We enforce namespacing at the host:

- Plugin declares a namespace, e.g. `acp`.
- Plugin registers local names (`toggle`, `start`, …).
- Host registers command as `acp.toggle`, `acp.start`, …

This guarantees uniqueness and makes collisions impossible.

Suggested structs:

```rust
#[repr(C)]
pub struct TomeCommandSpecV1 {
    pub name: TomeStr,          // local name: "toggle"
    pub aliases: TomeStrArray,  // local aliases: ["t"]
    pub description: TomeStr,

    pub handler: Option<extern "C" fn(ctx: *mut TomeCommandContextV1) -> TomeStatus>,
    pub user_data: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct TomeCommandContextV1 {
    pub argc: usize,
    pub argv: *const TomeStr,
    // term-only: allow a few editor operations directly (message, insert text, etc.)
    pub host: *const TomeHostV2,
}
```

### 3.5 Panel surface (term-only)

Panels are identified by an integer handle (`u64`).

```rust
pub type TomePanelId = u64;

#[repr(C)]
pub enum TomePanelKind {
    Chat = 1,
}

#[repr(C)]
pub struct TomeHostPanelApiV1 {
    pub create: extern "C" fn(kind: TomePanelKind, title: TomeStr) -> TomePanelId,
    pub set_open: extern "C" fn(id: TomePanelId, open: TomeBool),
    pub set_focused: extern "C" fn(id: TomePanelId, focused: TomeBool),
    pub append_transcript: extern "C" fn(id: TomePanelId, role: TomeChatRole, text: TomeStr),
    pub request_redraw: extern "C" fn(),
}

#[repr(C)]
pub enum TomeChatRole { User = 1, Assistant = 2, System = 3, Thought = 4 }
```

Important: plugins should *not* call these from background threads.

We keep these APIs so plugins can still “imperatively” act during `init`/command execution, but the intended fast-path is emitting events + host polling.

### 3.6 Permission request flow

ACP needs interactive approvals. Define a host-driven permission UI and a guest callback:

```rust
pub type TomePermissionRequestId = u64;

#[repr(C)]
pub struct TomePermissionOptionV1 {
    pub option_id: TomeOwnedStr,
    pub label: TomeOwnedStr,
}

#[repr(C)]
pub struct TomePermissionRequestV1 {
    pub prompt: TomeOwnedStr,
    pub options: *mut TomePermissionOptionV1,
    pub options_len: usize,
}
```

Two ways to route decisions:

A) Host calls guest callback:

- `guest.on_permission_decision(request_id, selected_option_id)`

B) Host emits a “decision event” the plugin polls

Prefer **A** (direct callback) because the UI decision is synchronous and already on the UI thread.

### 3.7 Event pump: the core bridge

The host will poll plugins for events once per UI tick.

```rust
#[repr(C)]
pub enum TomePluginEventKind {
    PanelAppend = 1,
    PanelSetOpen = 2,
    ShowMessage = 3,
    RequestPermission = 4,
}

#[repr(C)]
pub struct TomePluginEventV1 {
    pub kind: TomePluginEventKind,
    pub panel_id: TomePanelId,
    pub role: TomeChatRole,
    pub text: TomeOwnedStr,

    pub permission_request_id: TomePermissionRequestId,
    // ... plus a tagged union for the permission payload
}

#[repr(C)]
pub struct TomeGuestV2 {
    pub abi_version: u32,

    // Metadata
    pub namespace: TomeStr,  // e.g. "acp"
    pub name: TomeStr,       // human readable
    pub version: TomeStr,

    // Lifecycle
    pub init: Option<extern "C" fn(host: *const TomeHostV2) -> TomeStatus>,
    pub shutdown: Option<extern "C" fn()>,

    // Host-driven polling
    pub poll_event: Option<extern "C" fn(out: *mut TomePluginEventV1) -> TomeBool>,
    pub free_str: Option<TomeFreeStrFn>,

    // Callbacks from host
    pub on_panel_submit: Option<extern "C" fn(panel: TomePanelId, text: TomeStr)>,
    pub on_permission_decision: Option<extern "C" fn(id: TomePermissionRequestId, option_id: TomeStr)>,
}
```

Notes:

- `poll_event` returns `TomeBool(1)` when it wrote an event, `TomeBool(0)` when queue is empty.
- Events carry guest-owned allocations (`TomeOwnedStr`), freed by `guest.free_str`.

### 3.8 The host table itself

`TomeHostV2` aggregates smaller sub-APIs (panels, messaging, filesystem, etc.) to keep it readable.

```rust
#[repr(C)]
pub struct TomeHostV2 {
    pub abi_version: u32,

    // Diagnostics
    pub log: Option<extern "C" fn(msg: TomeStr)>,

    // UI + editor
    pub panel: TomeHostPanelApiV1,
    pub show_message: extern "C" fn(kind: TomeMessageKind, msg: TomeStr),
    pub insert_text: extern "C" fn(text: TomeStr),

    // Optional for ACP tooling (v1 MVP can omit)
    pub fs_read_text: Option<extern "C" fn(path: TomeStr, out: *mut TomeOwnedStr) -> TomeStatus>,
    pub fs_write_text: Option<extern "C" fn(path: TomeStr, content: TomeStr) -> TomeStatus>,
}
```

______________________________________________________________________

## 4) Host implementation plan (`tome-term`)

### 4.1 Add a PluginManager

Create a new module:

- `crates/tome-term/src/plugins/mod.rs`
- `crates/tome-term/src/plugins/manager.rs`
- `crates/tome-term/src/plugins/panels.rs` (generic panel models)

Responsibilities:

- Discover plugins (autoload paths)
- Load dynamic libs via `libloading`
- Bind v2 entrypoint and initialize guest
- Maintain registries:
  - `namespace → plugin handle`
  - `command name (full) → handler`
  - `panel_id → panel state + owner plugin`
- Per tick: `poll_events()` for each plugin and apply to `Editor` state

### 4.2 Integrate with `Editor::execute_command_line`

Current path: `crates/tome-term/src/editor/mod.rs` → `find_command(name)`.

New order:

1. If `PluginManager` has a matching runtime command, execute it.
1. Else fall back to `tome_core::ext::find_command`.

This allows runtime commands without touching the `linkme` system.

### 4.3 Generic panel UI (replace ACP-specific)

Refactor:

- `crates/tome-term/src/acp/types.rs` → becomes `crates/tome-term/src/plugins/panels.rs`
- `crates/tome-term/src/render/agent.rs` → becomes `crates/tome-term/src/render/plugin_panels.rs`

Panel type (host-owned):

- `ChatPanelState { open, focused, transcript, input, cursor }`

Host sends submitted input back to the plugin owning the panel:

- On Ctrl+Enter: `guest.on_panel_submit(panel_id, text)`

### 4.4 Auto-load plugins

Autoload should occur during startup, before the main loop.

Proposed search order (term-only, minimal deps):

1. `$TOME_PLUGIN_DIR` (single directory)
1. `$XDG_CONFIG_HOME/tome/plugins` (or `~/.config/tome/plugins`)
1. Optional: `.tome/plugins` under current working directory

Suggested file globs:

- Linux: `*.so`
- macOS: `*.dylib`
- Windows: `*.dll`

Implementation detail:

- Add `PluginManager::autoload()` invoked from `crates/tome-term/src/main.rs` or `run_editor` in `crates/tome-term/src/app.rs`.
- Autoload errors are non-fatal: show a message line + log.

### 4.5 Replace `:cabi_load` with term host load

Today `:cabi_load` lives in `tome-core` and loads plugins inside `tome-core` (`crates/tome-core/src/ext/commands/cabi.rs`). That cannot provide term-only host APIs.

Plan:

- Add `EditorOps::plugin_load(path)` to `crates/tome-core/src/ext/mod.rs`.
- Implement it in `crates/tome-term/src/editor/mod.rs` by delegating to `PluginManager`.
- Update `:cabi_load` handler to call `ctx.editor.plugin_load(path)`.

This keeps command registration in `tome-core` but moves runtime load to the correct layer.

______________________________________________________________________

## 5) ACP plugin crate plan (`tome-acp-plugin`)

### 5.1 New crate

Add workspace member:

- `crates/tome-acp-plugin` (`crate-type = ["cdylib"]`)

Dependencies (mirrors built-in ACP code):

- `agent-client-protocol` (already in workspace)
- `tokio`, `tokio-util`

### 5.2 Porting strategy

Start from current code:

- `crates/tome-term/src/acp/client.rs`

Port into plugin with these changes:

- Replace `std::sync::mpsc::Sender<AgentUiEvent>` with an internal `VecDeque<TomePluginEventV1>` guarded by `Mutex`.
- The ACP message handler enqueues `PanelAppend` events rather than calling UI directly.
- Store `panel_id` returned by host `panel.create(Chat, "Agent")`.

### 5.3 Namespaced commands

Plugin declares namespace `acp` and registers local commands:

- `toggle` → `acp.toggle`
- `start` → `acp.start`
- `stop` → `acp.stop`
- `insert_last` → `acp.insert_last`

The host prefixes and registers.

### 5.4 Input submission

Host routes Ctrl+Enter submissions to:

- `guest.on_panel_submit(panel_id, text)`

Plugin behavior:

- Append `User:` transcript event
- Clear input is host-owned; plugin does not mutate input state directly
- Send ACP `PromptRequest`

### 5.5 Permission requests

When ACP sends `session/request_permission`:

- Plugin allocates:
  - request prompt
  - option list
- Plugin emits `RequestPermission` event

Host UI renders a modal/inline prompt in the chat panel.

When user chooses:

- Host calls `guest.on_permission_decision(id, option_id)`
- Plugin resolves the pending oneshot and returns `RequestPermissionResponse`

______________________________________________________________________

## 6) Migration plan (phased)

### Phase 1: Build the v2 ABI + host plugin manager

- Implement `tome_plugin_entry_v2` discovery and new tables.
- Add `PluginManager` in `tome-term`.
- Add generic chat panel UI and event pump.
- Upgrade `demo-cabi-plugin` to v2 to validate:
  - command registration
  - transcript append events
  - panel submit callback

Exit criteria:

- `:demo.hello` (or similar) works.
- A panel can open and show lines emitted by the plugin.

### Phase 2: ACP plugin MVP

- Implement `tome-acp-plugin`:
  - start/stop agent
  - send prompt
  - stream transcript updates

Exit criteria:

- `acp.start` and `acp.toggle` fully replace `:agent_start`/`:agent`.

### Phase 3: Permissions + tools

- Add permission UI plumbing.
- Implement safe fs/terminal tool routing (either in host or plugin).

Exit criteria:

- No more “always allow” permission stub.
- Controlled file writes and terminal execution prompts.

### Phase 4: Remove built-in ACP

- Delete or gate `crates/tome-term/src/acp/*`.
- Remove ACP-specific commands from `tome-core` if superseded.

______________________________________________________________________

## 7) Detailed task breakdown

### 7.1 `tome-cabi-types`: v2 types + header generation

- [ ] Add `TomeStr`, `TomeOwnedStr`, `TomeBool`, and event/command structs
- [ ] Add `TomeHostV2`, `TomeGuestV2`, and `TomePluginEntryV2` signature
- [ ] Keep v1 structs intact
- [ ] Ensure `cbindgen` output remains stable (`crates/tome-cabi-types/build.rs`)

### 7.2 `tome-term`: plugin runtime

- [ ] Implement `PluginManager::autoload()` with directory search
- [ ] Implement load + symbol resolution with `libloading`
- [ ] Implement namespaced runtime command registry
- [ ] Update `Editor::execute_command_line` to consult plugins first
- [ ] Add generic panel renderer + input routing
- [ ] Add permission prompt rendering (panel overlay)
- [ ] Add per-tick event pumping (`poll_events`) from plugins

### 7.3 `demo-cabi-plugin`: v2 demo

- [ ] Export `tome_plugin_entry_v2`
- [ ] Register a namespaced command (`demo.hello`)
- [ ] Create a chat panel and append a transcript line

### 7.4 `tome-acp-plugin`: ACP-as-plugin

- [ ] Port ACP runtime from `crates/tome-term/src/acp/client.rs`
- [ ] Implement command handlers (`toggle/start/stop/insert_last`)
- [ ] Implement `poll_event` queueing and `free_str`
- [ ] Implement `on_panel_submit` to send prompts
- [ ] Implement permission request/decision flow

______________________________________________________________________

## 8) Code sketch: host-side load + dispatch

```rust
// crates/tome-term/src/plugins/manager.rs
pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
    commands: HashMap<String, PluginCommand>,
    panels: HashMap<u64, ChatPanelState>,
}

impl PluginManager {
    pub fn autoload(&mut self) {
        for dir in discover_plugin_dirs() {
            for path in list_dynamic_libs(dir) {
                if let Err(e) = self.load(&path) {
                    // show message + log
                }
            }
        }
    }

    pub fn try_execute(&mut self, full_name: &str, args: &[&str], ed: &mut Editor) -> bool {
        let Some(cmd) = self.commands.get(full_name).cloned() else { return false; };
        cmd.invoke(args, ed);
        true
    }

    pub fn poll_events(&mut self, ed: &mut Editor) {
        for plugin in &mut self.plugins {
            while let Some(evt) = plugin.poll_event() {
                self.apply_event(evt, ed, plugin);
            }
        }
    }
}
```

______________________________________________________________________

## 9) References (files to consult while implementing)

- C-ABI loader: `crates/tome-core/src/ext/plugins/cabi.rs`
- C-ABI types: `crates/tome-cabi-types/src/lib.rs`
- Header generation: `crates/tome-cabi-types/build.rs`
- Command parsing/dispatch: `crates/tome-term/src/editor/mod.rs`
- Existing ACP runtime: `crates/tome-term/src/acp/client.rs`
- Existing ACP panel render: `crates/tome-term/src/render/agent.rs`
- ACP integration handover: `ACP_INTEGRATION_HANDOVER.md`

______________________________________________________________________

## 10) Acceptance criteria

- Tome starts and auto-loads plugins from config directory.
- Loaded plugins can register namespaced commands, listed in help (optional) and executable.
- A plugin can open a chat panel, append transcript lines, and receive user submissions.
- ACP plugin can start `opencode acp`, stream output, and request permissions.

______________________________________________________________________

## 11) Implementation notes / pitfalls

- Never call UI-mutating host functions from plugin background threads; funnel everything through events.
- Ensure dynamic library lifetimes: store `Library` alongside guest vtables.
- Keep ABI structs `#[repr(C)]` and avoid Rust enums unless explicitly `#[repr(C)]` with fixed discriminants.
- Be careful with string ownership; always define who frees.
- Command namespace enforcement belongs in the host.
