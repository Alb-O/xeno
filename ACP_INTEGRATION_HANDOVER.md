# Tome ↔ ACP (Agent Client Protocol) Integration Handover

This document is a detailed technical handover for integrating the Agent Client Protocol (ACP) into Tome’s terminal UI editor (`tome-term`), with **OpenCode** (`opencode acp`) as the initial agent implementation.

It is written to be implementation-ready: it inventories Tome’s current architecture, proposes a concrete design, and specifies testable milestones.

---

## 1. Scope and goals

### 1.1 What we’re building

- **Tome is the ACP client**.
- **`opencode acp` is the ACP agent** (subprocess spawned by Tome).
- ACP transport for this pairing is **newline-delimited JSON-RPC 2.0 over stdio** (NDJSON / “ndjson stream”).

### 1.2 MVP definition (first usable version)

A user can:

1. Start/stop an ACP agent session from within Tome.
2. Send a prompt (ideally multiline) from Tome.
3. See streaming agent output in the TUI.
4. Insert the last assistant message into the current buffer.
5. Cancel an in-flight agent turn.

### 1.3 Next milestone (first “editor-integrated” version)

A user can:

1. Review tool calls and diffs.
2. Approve/deny permissions for file writes and command execution.
3. Have the agent read/write files within a workspace root.

---

## 2. ACP refresher (what matters for Tome)

### 2.1 ACP is symmetric JSON-RPC

ACP is a JSON-RPC based protocol. The key idea is:

- The **client** sends requests like `initialize`, `session/new`, `session/prompt`, `session/cancel`.
- The **agent** sends:
  - notifications `session/update` streaming progress (message chunks, tool calls, plan), and
  - requests back to the client for file operations and permissions (e.g. `fs/read_text_file`, `fs/write_text_file`, `terminal/*`, `session/request_permission`).

In the Rust schema crate (cloned here as `agent-client-protocol-schema`), these are represented by:

- `ClientRequest` / `AgentResponse` (client → agent)
- `AgentRequest` / `ClientResponse` (agent → client)
- `AgentNotification` / `ClientNotification` (notifications)

Useful references:

- Protocol transport: `agent-client-protocol/docs/protocol/transports.mdx`
- Prompt lifecycle types: `agent-client-protocol/src/agent.rs:2024` (PromptRequest/Response)
- Streaming updates: `agent-client-protocol/src/client.rs:77` (SessionUpdate)
- Tool call types: `agent-client-protocol/src/tool_call.rs:24` (ToolCall/ToolCallUpdate)

### 2.2 Transport constraints (critical)

ACP stdio framing rules (per spec):

- Each JSON-RPC message is one JSON object.
- Messages are delimited by `\n`.
- Messages **must not contain embedded newlines**.

This matters for Tome because we must ensure:

- We write exactly one JSON object per line.
- We do not accidentally pretty-print JSON.
- We handle partial reads and join bytes until newline.

### 2.3 The Rust runtime crate vs schema crate

Two crates exist in the ACP ecosystem:

1. `agent-client-protocol-schema` (this repo):
   - Contains the protocol types and method name constants.
   - Does **not** provide high-level connection/transport orchestration.

2. `agent-client-protocol` (published on crates.io):
   - Provides `ClientSideConnection` / `AgentSideConnection` and a transport-agnostic async connection layer.

For Tome, the easiest implementation is to depend on `agent-client-protocol` and use:

- `ClientSideConnection::new(handler, outgoing, incoming, spawn)`

This returns:

- a connection object implementing the *client-facing Agent API* (`initialize`, `new_session`, `prompt`, etc.)
- a future that must be driven/spawned to pump IO

Important nuance from the crate’s API:

- The connection constructor takes a `MessageHandler<ClientSide>`.
- You can implement `MessageHandler` directly and route `AgentRequest`/`AgentNotification` yourself.

---

## 3. OpenCode ACP behavior (the initial agent)

OpenCode’s ACP command (`opencode acp`) is implemented in:

- `opencode/packages/opencode/src/cli/cmd/acp.ts`

Key observations:

- It wraps stdin/stdout into a `ndJsonStream(...)` and instantiates an ACP **AgentSideConnection**.
- It also starts a local HTTP server for its own internal SDK communication (this is why `--port/--hostname` exist). ACP itself still uses stdio.

### 3.1 The “OPENCODE_LIBC is not defined” log

When running `opencode acp` in this environment, you may see logs like:

- `OPENCODE_LIBC is not defined`

Root cause:

- OpenCode’s bundling expects `OPENCODE_LIBC` to be a compile-time constant used to choose a `@parcel/watcher` binding.
- It is declared as `declare const OPENCODE_LIBC: string | undefined` in `opencode/packages/opencode/src/file/watcher.ts`.
- This is not an environment variable and cannot be fixed by `env OPENCODE_LIBC=...`.

Impact:

- This appears to be a non-fatal unhandled rejection in OpenCode’s file watcher initialization.
- The ACP command still prints `setup connection` afterwards.

Recommendation:

- Do not block Tome integration on this log.
- Ensure Tome tolerates stderr noise from the agent subprocess.

---

## 4. Tome current architecture (where ACP must hook in)

### 4.1 Process entrypoint and event loop

- `tome/crates/tome-term/src/main.rs` creates an `Editor` and calls `run_editor(editor)`.
- `tome/crates/tome-term/src/app.rs` contains the main loop:
  - `terminal.draw(|frame| editor.render(frame))`
  - reads events from `termina` and calls `editor.handle_key`, `editor.handle_mouse`, etc.

Important detail:

- The loop blocks waiting for terminal events in non-insert mode:
  - it only uses `poll(Some(Duration::from_millis(50)))` in insert mode
  - otherwise it effectively does a blocking read

This matters because ACP is streaming: the UI needs periodic wakeups to redraw when agent output arrives.

### 4.2 Editor state and rendering

- `Editor` is in `tome/crates/tome-term/src/editor/mod.rs`.
- Rendering is implemented in `tome/crates/tome-term/src/render/document.rs`:
  - Main document
  - Status line (`render/status.rs`)
  - Message line (`render/message.rs`)
  - Scratch popup overlay (command scratch)

Scratch popup is already a bottom-docked overlay with its own buffer and focus management.

### 4.3 Command execution and extensibility

Tome has two relevant “command surfaces”:

1. **Action system** (keybindings → actions → `ActionResult` → result handlers)
   - Keybinding registry in `tome/crates/tome-core/src/ext/keybindings/*`.
   - Dispatch in `tome/crates/tome-core/src/ext/editor_ctx/handlers.rs:106`.

2. **Ex/command line commands** (typed commands like `:write`)
   - Registered in `tome/crates/tome-core/src/ext/commands/*`.
   - Looked up via `tome_core::ext::find_command(...)`.
   - Executed by `Editor::execute_command_line` in `tome/crates/tome-term/src/editor/mod.rs:64`.

The scratch popup currently acts like a multiline command palette: it executes its contents as a command line when the user presses Enter/Ctrl+Enter.

---

## 5. High-level design options

There are multiple ways to integrate ACP into the existing UI and extension system. The recommended path is to minimize invasive refactors while still supporting streaming.

### Option A (recommended): Add a dedicated Agent panel (separate from scratch)

Pros:

- Clear UX separation between command scratch and agent chat.
- Can represent streaming output, tool calls, permissions, and diffs cleanly.
- Minimizes surprising changes to existing scratch semantics.

Cons:

- Requires new state + rendering + input routing.

### Option B (fastest MVP): Reuse scratch popup as “agent prompt input” with a mode switch

Idea:

- Add `scratch_mode: Command | Agent` in `tome-term::Editor`.
- Introduce a command `:agent` that switches scratch into Agent mode and opens it.
- When executing scratch while in Agent mode, send ACP prompt instead of running command.

Pros:

- Very small UI surface area.
- Leverages existing multiline input and focus management.

Cons:

- Scratch buffer becomes overloaded (command input vs chat transcript).
- More difficult to show rich tool call UI.

### Recommendation

Use Option B for a *quick “it talks” demo*, but plan to migrate to Option A shortly after.

This document proceeds with **Option A** as the long-term architecture, but includes an “MVP shortcut” section describing Option B.

---

## 6. Proposed architecture (Option A)

### 6.1 New modules (in `tome-term`)

Add a new module tree:

- `tome/crates/tome-term/src/acp/`
  - `mod.rs` (public facade)
  - `client.rs` (ACP client state machine + subprocess management)
  - `protocol.rs` (type conversions, helper constructors)
  - `ui.rs` (Agent panel model: messages, tool calls, selection insertion)
  - `permissions.rs` (permission prompt state machine)
  - `terminal.rs` (implementation of ACP `terminal/*` requests)

### 6.2 Editor-level integration points

Modify `Editor` (term layer) to hold:

- `agent_panel: AgentPanelState`
- `agent_runtime: Option<AcpClientRuntime>`

Where:

- `AgentPanelState` is purely UI state (open/focus/input/scroll/messages)
- `AcpClientRuntime` is the protocol “engine” (spawned subprocess, channels)

### 6.3 Concurrency model

Tome’s main loop is synchronous. ACP communication is naturally async.

To bridge this:

1. Spawn a dedicated background thread that runs a Tokio runtime.
2. That thread:
   - spawns the `opencode acp` subprocess
   - connects stdio to ACP `ClientSideConnection`
   - runs the connection IO future
   - sends events to the UI thread via `std::sync::mpsc` channel
3. UI thread:
   - checks `try_recv()` each frame/tick
   - updates `AgentPanelState`
   - triggers redraw by ensuring the main loop polls periodically

### 6.4 UI wakeups (important)

Because the main loop currently blocks in normal mode, streaming updates will not render unless the user presses keys.

Fix options:

- **Recommended**: always call `events.poll(Some(Duration::from_millis(TICK_MS)))` with a small tick (e.g. 50ms) in *all* modes. If no terminal event, still render and process ACP events.

This is the minimum change to make streaming usable.

---

## 7. ACP engine design (`AcpClientRuntime`)

### 7.1 Responsibilities

`AcpClientRuntime` is responsible for:

- spawning and killing the agent subprocess
- maintaining a single ACP connection + session id
- providing a command API to the UI thread:
  - start
  - stop
  - send prompt
  - cancel
- handling incoming agent requests/notifications:
  - session/update streaming → UI events
  - session/request_permission → UI prompt
  - fs/terminal methods → implement locally

### 7.2 Minimal message contract between engine and UI

Define enums:

```rust
enum AgentUiEvent {
  Connected { agent_name: String, protocol_version: String },
  Disconnected { reason: String },

  SessionStarted { session_id: String },
  SessionUpdate(SessionUpdate),

  PermissionRequested { request_id: String, details: PermissionRequestView },

  ToolingError { message: String },
}

enum AgentCommand {
  Start { cwd: PathBuf },
  Stop,
  Prompt { content: String, context: PromptContext },
  Cancel,
  PermissionDecision { request_id: String, option_id: String },
}
```

Notes:

- Keep UI-facing events high-level and stable.
- Use ACP schema types internally, but consider translating to “view models” so the UI does not depend on ACP crate types everywhere.

### 7.3 Subprocess management

Spawn command:

- `opencode acp --port 0 --hostname 127.0.0.1` (the defaults are fine)

Implementation details:

- Use `std::process::Command`.
- Set:
  - `stdin(Stdio::piped())`
  - `stdout(Stdio::piped())`
  - `stderr(Stdio::inherit())` or capture; for MVP inherit is fine.

### 7.4 Using the Rust ACP runtime crate

In `tome-term`, add dependencies:

- `agent-client-protocol = "<version>"`
- `tokio = { version = "<version>", features = ["rt-multi-thread", "process", "io-util", "sync", "macros"] }`

Then:

1. Convert the child stdio handles into tokio AsyncRead/AsyncWrite.
2. Implement a `MessageHandler<ClientSide>` that:
   - handles `AgentRequest` → `ClientResponse`
   - handles `AgentNotification` → `Result<()>`

3. Call `ClientSideConnection::new(handler, child_stdin, child_stdout, spawn)`

4. Drive the IO future by spawning it onto the runtime.

5. Use the returned `ClientSideConnection` (it implements the `Agent` API) to call:

- `initialize(InitializeRequest)`
- `new_session(NewSessionRequest)`
- `prompt(PromptRequest)`
- `cancel(CancelNotification)`

### 7.5 Initialization handshake

1. `initialize`

- Provide `protocol_version` and `client_capabilities`.
- Start with conservative capabilities:
  - `fs.read_text_file = true`
  - `fs.write_text_file = true` (if you want agent to apply edits)
  - `terminal = true` (OpenCode likely uses terminal tools)

2. `session/new`

- Provide `cwd` (workspace root).
- Provide `mcp_servers` empty initially.

Store returned `session_id`.

### 7.6 Prompt flow

For each user prompt:

- Build `PromptRequest { session_id, prompt: vec![ContentBlock::Text(TextContent{...})] }`
- Call `prompt(...)`.

While the prompt runs, the agent will send `session/update` notifications.

When complete, `prompt(...)` returns `PromptResponse { stop_reason }`.

### 7.7 Cancellation

- On user cancel, send `session/cancel` notification.
- Continue to process `session/update` notifications after cancellation (protocol recommends this).

---

## 8. Implementing client-side ACP methods (agent → client)

These arrive as `AgentRequest` and must yield a `ClientResponse`.

### 8.1 `session/request_permission`

When received:

- Emit `AgentUiEvent::PermissionRequested` to the UI thread.
- The ACP engine should await a user response (via `AgentCommand::PermissionDecision`).

Implementation approach:

- Create a per-request oneshot channel stored in a map keyed by request id.
- `handle_request` blocks (async) awaiting the oneshot.
- When UI replies, fill and return `RequestPermissionResponse`.

Safety policy recommendation:

- Default deny for:
  - write_text_file outside workspace root
  - terminal/create that runs arbitrary commands
- Default allow for:
  - read_text_file within workspace

Persist “allow always” and “reject always” in memory for the session; later extend to config.

### 8.2 `fs/read_text_file`

Implement by:

- verifying the requested path is allowed (workspace root enforcement)
- reading file content
- applying line/limit (ACP request uses 1-based line numbers)

Return `ReadTextFileResponse { content }`.

### 8.3 `fs/write_text_file`

Implement by:

- verifying path is allowed
- writing content atomically (recommended): write temp file then rename
- updating the open buffer if it corresponds to the current file

UI integration options:

- If the file is currently open in Tome, apply the update to `Editor.doc` and mark `modified = true`.
- If it’s not open, write to disk only.

### 8.4 `terminal/*` methods

OpenCode commonly uses terminal tool calls.

Minimum viable implementation:

- `terminal/create`: spawn a process, capture stdout+stderr (merged), return a `TerminalId`.
- `terminal/output`: return buffered output so far and exit status.
- `terminal/wait_for_exit`: block until exit, then return status.
- `terminal/kill`: send kill signal.
- `terminal/release`: free resources.

Implementation detail:

- Maintain a map `TerminalId -> TerminalProcessState` in the ACP engine.
- Each terminal process should have a background task reading its output into a ring buffer.

Note: this is “agent tooling” not Tome’s embedded terminal. For MVP, returning captured output strings is enough.

---

## 9. Agent panel UI design

### 9.1 UI state

A good MVP UI model:

```rust
struct AgentPanelState {
  open: bool,
  focused: bool,

  // input area
  input: Rope,
  input_cursor: usize,

  // conversation transcript
  transcript: Vec<ChatItem>,

  // tool call timeline
  tool_calls: HashMap<ToolCallId, ToolCall>,

  // last assistant message convenience
  last_assistant_text: String,

  // permission prompt (if any)
  permission_prompt: Option<PermissionPromptState>,
}
```

`ChatItem` should represent streaming chunks:

- user message
- assistant message (appended as chunks)
- optional “thought” chunks (hidden by default)

### 9.2 Rendering approach

Reusing the existing rendering pattern:

- main doc remains unchanged
- add an overlay that renders when `agent_panel.open`

Layout proposal:

- Bottom-docked panel consuming, say, 40% of height.
- Top portion: scrollable transcript
- Bottom portion: multiline input editor

You already have code for a bottom-docked scratch popup in `tome/crates/tome-term/src/render/document.rs:67`.

You can generalize it:

- Extract “render popup overlay” into a helper that takes a buffer view.
- Implement the agent panel as another overlay.

### 9.3 Input routing

Mirror the scratch focus logic in `Editor::handle_key` (`tome/crates/tome-term/src/editor/mod.rs:468`).

For agent panel:

- If agent panel focused:
  - in insert mode, characters insert into agent input buffer
  - Ctrl+Enter sends prompt
  - Esc exits insert mode or closes panel (two-step like scratch)

### 9.4 Commands and keybindings

Start with Ex commands:

- `:agent` → open/close agent panel
- `:agent_start` → explicitly spawn agent
- `:agent_stop` → kill agent subprocess
- `:agent_send` → send current selection / current file context

Keybinding recommendations (normal mode):

- Bind `Ctrl+A` or `Alt+A` to toggle agent panel (choose something non-conflicting)

Implementation options:

- Implement as **commands** in `tome-core` that call new methods on `EditorOps`.
- Or implement as **actions** with new `ActionResult` variants + capability traits.

For MVP, commands are often simpler to wire.

---

## 10. MVP shortcut (Option B: reuse scratch as agent input)

If you want a very fast “it works” demo:

1. Add `scratch_mode: ScratchMode` to `tome-term::Editor`:

```rust
enum ScratchMode { Command, Agent }
```

2. Add a new Ex command `:agent` that:

- sets `scratch_mode = Agent`
- opens scratch with focus

3. Change `do_execute_scratch()`:

- if `scratch_mode == Command`: existing behavior
- if `scratch_mode == Agent`: treat scratch text as prompt and send ACP `session/prompt`

4. Append agent output into the message line or into scratch.

This gets basic prompting quickly, but will likely be replaced later.

---

## 11. Testing plan

Tome already has good TUI unit testing patterns in:

- `tome/crates/tome-term/src/tests.rs`

They use:

- `ratatui::backend::TestBackend`
- snapshot testing via `insta`

### 11.1 Unit tests for UI state updates

Add tests that:

- open agent panel
- simulate receipt of `SessionUpdate::AgentMessageChunk`
- ensure transcript updates and render snapshot changes

This can be done without any subprocess.

### 11.2 Protocol tests with an in-process mock agent

Instead of spawning OpenCode, create a tiny mock ACP agent in Rust tests:

- Use `tokio::io::duplex(...)` to create in-memory streams.
- Run a minimal agent-side loop that:
  - responds to `initialize` and `session/new`
  - for `session/prompt` sends a few `session/update` notifications then returns `StopReason::EndTurn`

Then test:

- Tome ACP engine receives updates and forwards them to UI channel
- cancellation works (returns `StopReason::Cancelled`)

### 11.3 Permission tests

Mock agent sends:

- `session/request_permission` for a fake tool call

Test:

- UI receives prompt event
- sending a decision yields the correct ACP response

### 11.4 Real-world smoke test with OpenCode

Manual steps:

1. Run Tome in a repo
2. Start agent panel
3. Send prompt: “Summarize current file”
4. Verify streaming appears
5. Cancel mid-stream and verify it stops
6. Ask for a small edit and confirm permission UI

---

## 12. Implementation checklist (step-by-step)

### Step 1: Make the UI tick even without input

- Modify `tome/crates/tome-term/src/app.rs` main loop to poll with a timeout always.
- Each tick:
  - drain ACP UI events (non-blocking)
  - redraw

This is required for streaming.

### Step 2: Introduce ACP engine skeleton

- Add `tome-term/src/acp/` module and a minimal `AcpClientRuntime`.
- Implement subprocess spawn/stop.
- Add a channel pair (UI → engine, engine → UI).

### Step 3: Implement `initialize` + `session/new`

- On start, call `initialize` then `session/new`.
- Emit events to UI.

### Step 4: Implement prompt sending

- Send `PromptRequest` with a text block.
- Stream updates into UI.

### Step 5: Implement agent panel UI

- Add open/focus state
- render transcript
- send prompt on Ctrl+Enter

### Step 6: Add “insert last answer”

- Track last assistant message text
- Add a command/keybinding to insert into `Editor.doc` via existing `insert_text`.

### Step 7: Implement permissions + fs read/write

- request_permission UI
- enforce workspace root

### Step 8: Implement terminal methods

- minimal terminal process runner

---

## 13. Known pitfalls and edge cases

- **Newline-delimited JSON**: never pretty-print messages; avoid embedded newlines.
- **UI starvation**: without a periodic tick, streaming won’t display.
- **Workspace root enforcement**: always validate paths.
- **Concurrency**: keep all `Editor` mutations on the UI thread.
- **Backpressure**: if the agent sends many updates quickly, bound channel sizes or coalesce updates.
- **Session lifecycle**: handle agent disconnects gracefully and surface errors.

---

## 14. File references (for quick navigation)

- Main loop: `tome/crates/tome-term/src/app.rs:16`
- Editor state: `tome/crates/tome-term/src/editor/mod.rs:22`
- Scratch popup rendering: `tome/crates/tome-term/src/render/document.rs:67`
- Core action dispatch: `tome/crates/tome-core/src/ext/editor_ctx/handlers.rs:106`
- ACP prompt types: `agent-client-protocol/src/agent.rs:2024`
- ACP session updates: `agent-client-protocol/src/client.rs:77`
- ACP tool calls: `agent-client-protocol/src/tool_call.rs:24`
- OpenCode ACP command: `opencode/packages/opencode/src/cli/cmd/acp.ts:18`

---

## 15. Suggested next action

If you want to begin implementation immediately, start with:

1. Add a periodic tick in `tome-term` main loop.
2. Add the ACP engine module and a dummy “agent panel” that just displays a hardcoded line.
3. Wire prompt sending to a mock agent via duplex streams in tests.

That sequence ensures you have a fast red/green loop with reliable tests before introducing the real `opencode acp` subprocess.
