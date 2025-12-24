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

## 2. Host Extensions (`tome-extensions`)

Host extensions define the **environment of the editor**. They handle stateful services, UI components, and integration with the host operating system. These depend on `tome-api`.

- **Responsibilities**:
  - **State Management**: Storing persistent data (e.g., ACP chat history, LSP client state) using the `ExtensionMap`.
  - **UI Panels**: Registering custom views (e.g., Chat panel, File tree).
  - **Lifecycle**: Hooking into the editor heartbeat via `TICK_EXTENSIONS`.
  - **Initialization**: Populating the `ExtensionMap` during `Editor` creation.
- **Characteristics**:
  - **Stateful**: They inject their own types into the `Editor.extensions` TypeMap.
  - **Host-Specific**: They depend on the editor Engine API (`tome-api`).
  - **Modular**: Built as a separate crate to avoid circular dependencies with the CLI runner.

## 3. Dependency Direction & Coupling

To maintain stability and testability, dependency directions are strictly enforced:

1. **Runner -> Extensions -> API -> Core**: `tome-term` (the runner in `crates/term`) depends on `tome-extensions` (in `crates/extensions`), which depends on `tome-api` (the engine in `crates/api`), which depends on `tome-core` (in `crates/core`).
1. **Core -X API -X Extensions**: `tome-core` must **never** depend on higher-level crates.
1. **Internal Decoupling**: The `Editor` struct (in `tome-api`) does not know about specific extensions. It only knows about the `ExtensionMap`. Extensions register themselves via the `EXTENSIONS` registry defined in `tome-api`.

### Summary Table

| Feature        | Core Builtin Extension    | Host Plugin                            |
| -------------- | ------------------------- | -------------------------------------- |
| **Crate**      | `tome-core`               | `tome-extensions`                      |
| **API Crate**  | N/A                       | `tome-api`                             |
| **Logic Type** | Functional / Pure         | Stateful / Side-effectful              |
| **Discovery**  | `linkme` (Global)         | `linkme` (Local to host)               |
| **Examples**   | `move_line_down`, `:quit` | `AcpManager`, `LspClient`, `ChatPanel` |

## 4. Stable Interface (`tome-api`)

The `tome-api` crate serves as the bridge between the editor engine and its extensions. It contains:

- The `Editor` struct and its public operations.
- UI traits like `Panel`.
- Theme definitions.
- The `ExtensionMap` and registration slices (`EXTENSIONS`, `TICK_EXTENSIONS`).

This decoupling ensures that adding a new UI panel to an extension doesn't require modifying the core engine or the CLI runner.
