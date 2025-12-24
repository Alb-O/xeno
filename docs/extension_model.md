# Tome Extension Model

Tome uses a two-tier extension model to preserve orthogonality and maintain a clean boundary between core editing logic and host-specific features (like TUI, GUI, or LSP).

## 1. Core Builtins (`tome-core`)

Core builtins define the **language of the editor**. They are primarily registered via `distributed_slice` at compile-time and are handled by the `RegistryIndex`.

- **Responsibilities**:
  - **Actions**: High-level editor operations (e.g., `delete_char`, `insert_newline`).
  - **Commands**: Ex-mode commands (e.g., `:write`, `:quit`).
  - **Motions**: Selection movement logic (e.g., `move_left`, `next_word_start`).
  - **Text Objects**: Selection targets (e.g., `word`, `parentheses`).
  - **File Types**: Detection logic for language-specific settings.
- **Characteristics**:
  - **Stateless**: They operate on the provided `ActionContext` or `CommandContext`.
  - **Portable**: They do not depend on any specific UI or terminal implementation.
  - **Static Registration**: Collected into static slices (e.g., `ACTIONS`, `COMMANDS`).

## 2. Host Extensions (`tome-term`)

Host extensions define the **environment of the editor**. They handle stateful services, UI components, and integration with the host operating system.

- **Responsibilities**:
  - **State Management**: Storing persistent data (e.g., ACP chat history, LSP client state) using the `ExtensionMap`.
  - **UI Panels**: Registering custom views (e.g., Chat panel, File tree).
  - **Lifecycle**: Hooking into the editor heartbeat via `TICK_EXTENSIONS`.
  - **Initialization**: Populating the `ExtensionMap` during `Editor` creation.
- **Characteristics**:
  - **Stateful**: They inject their own types into the `Editor.extensions` TypeMap.
  - **Host-Specific**: They may depend on specific UI frameworks (e.g., Ratatui, Termina).
  - **Auto-Discovered**: In `tome-term`, these are discovered at build-time from the `extensions/` directory.

## 3. Dependency Direction & Coupling

To maintain stability and testability, dependency directions are strictly enforced:

1. **Host -> Core**: Host implementations (`tome-term`) and Host extensions depend on `tome-core`.
1. **Core -X Host**: `tome-core` must **never** depend on `tome-term` or any host-specific extensions.
1. **Internal Decoupling**: The `Editor` struct (in `tome-term`) does not know about specific extensions. It only knows about the `ExtensionMap`. Extensions register themselves via the `EXTENSIONS` registry.

### Summary Table

| Feature        | Core Builtin Extension                                        | Host Plugin                            |
| -------------- | ------------------------------------------------------------- | -------------------------------------- |
| **Crate**      | `tome-core`                                                   | `tome-term` (Host)                     |
| **Storage**    | `RegistryIndex` (Static) ExtensionuginMap\` (Runtime TypeMap) |                                        |
| **Logic Type** | Functional / Pure                                             | Stateful / Side-effectful              |
| **Discovery**  | `linkme` (Global)                                             | `build.rs` + `linkme` (Local)          |
| **Examples**   | `move_line_down`, `:quit`                                     | `AcpManager`, `LspClient`, `ChatPanel` |

## 4. Best Practices

- **Prefer Builtins**: If a feature can be implemented as a stateless action or command, put it in `tome-core`.
- **Use TypeMap for State**: Never add extension-specific fields to the `Editor` struct. Use `editor.extensions.insert(MyState::new())` during init.
- **Poll Sparingly**: Only use `TICK_EXTENSIONS` if the extension truly needs to perform background work or event polling on every frame.

## 5. Future: External Plugin Crates

If the project grows to support third-party plugins as independent crates, a `tome-api` crate will be introduced. This crate will contain the stable interfaces (traits and TypeMap keys) that both `tome-term` (the host) and the plugins depend on, breaking the potential circular dependency.
