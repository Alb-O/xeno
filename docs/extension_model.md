# Tome Extension Model (Compile-Time Only)

Tome uses a compile-time extension model with a suckless-ish philosophy: everything is linked in, no dynamic plugins, and crate boundaries are intentionally unstable.

## Principles

- Compile-time extensions only. No ABI, no dynamic loading, no hot reload.
- Keep core logic free of host or UI dependencies.
- Push heavy deps (tokio, pty, ratatui) to the integration layer and binaries.
- Accept instability: APIs and crate boundaries may move.

## 1. Core Builtins (tome-stdlib)

Core builtins define the language of the editor and are registered at compile time.

- Responsibilities:
  - Actions: high-level editor operations (delete, insert, transform).
  - Commands: ex-style commands (write, quit, etc.).
  - Motions: cursor/selection movement logic.
  - Text Objects: selection targets (word, paragraph, brackets).
  - Filetypes: language detection helpers.
- Characteristics:
  - Stateless: operate on ActionContext/CommandContext traits.
  - Portable: no UI or host-specific types.
  - Static registration via linkme/distributed_slice.
- Registries:
  - The linkme registries live in `tome-manifest`.

## 2. Host Extensions (tome-extensions)

Host extensions define the environment of the editor and are also registered at compile time.

- Responsibilities:
  - Stateful services (LSP, agentfs, background tasks).
  - UI panels and host-specific UI glue.
  - Editor lifecycle hooks (ticks, startup registration).
- Characteristics:
  - Stateful: store data in ExtensionMap.
  - Host-specific: depend on the integration layer.
- Dependencies:
  - `tome-extensions` depends on `tome-api` because ticks/panels require `&mut Editor`.

## 3. Registry Boundaries

- `tome-manifest` owns registries for builtins (actions, commands, hooks, motions, objects, options, statusline, filetypes).
- `tome-api` owns host extension registries that require `Editor` access (ticks, panels).
- `tome-stdlib` and `tome-extensions` are the primary implementers.

## 4. Dependency Direction (Current Plan)

```
[tome-term] (bin)
  -> [tome-api] (integration/app: Editor + UI + runtime + render)
       -> [tome-extensions] (host extensions; depends on tome-api)
       -> [tome-stdlib] (core builtins)
       -> [tome-input]
       -> [tome-language]
       -> [tome-theme]
       -> [tome-manifest]
       -> [tome-base]
       -> [tome-macro]
```

## 5. Summary Table

| Feature    | Core Builtins         | Host Extensions           |
| ---------- | --------------------- | ------------------------- |
| Crate      | tome-stdlib           | tome-extensions           |
| Registry   | tome-manifest         | tome-api                  |
| Logic Type | Functional / pure     | Stateful / side-effectful |
| Discovery  | linkme (compile-time) | linkme (compile-time)     |
| Examples   | move_line_down, :quit | LspClient, ChatPanel      |

## 6. Notes on Future Splits

Splitting `tome-api` into render/runtime/extension-api crates is deferred until extensions no longer need full `Editor` access and rendering is driven from a render model instead of editor methods.
