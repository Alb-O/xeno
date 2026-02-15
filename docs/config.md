# Xeno config formats

Xeno supports two user config entrypoints, loaded in this order:

1. `~/.config/xeno/config.nuon`
2. `~/.config/xeno/config.nu`

Later files win when fields overlap.

Options and keys merge with later layers overriding earlier values. Language entries are accumulated, and later entries for the same language take precedence when applied.

## Precedence

| Layer | File | Precedence |
| --- | --- | --- |
| 1 | `config.nuon` | base |
| 2 | `config.nu` | highest |

`config.nu` must evaluate to a record that follows the same schema as `config.nuon`.

## Runtime reload

Use `:reload-config` (or `:reload_config`) to reload config files without restarting the editor.

- re-reads `config.nuon` and `config.nu` in precedence order
- replaces current key overrides and option layers with the newly loaded config state
- logs per-file warnings and errors, then refreshes theme loading

## Nu macros

Xeno can run user-defined Nu macro functions from `~/.config/xeno/xeno.nu`.

* `:nu-reload` reloads and revalidates `xeno.nu`
* `:nu-run <fn> [args...]` runs an exported function and dispatches its output as invocations

`nu-run` expects the function to return one of:

* `null` / nothing: no-op (returns success)
* a single invocation (from a built-in command)
* a list of invocations and/or nothing values

Records and strings are **not** accepted as runtime return values. Use the built-in commands below.

### Built-in commands

Xeno registers native commands into every engine state. No `use` statement needed:

* `action <name> [--count N] [--extend] [--register R] [--char C]` — action invocation
* `command <name> [...args]` — registry command invocation
* `editor <name> [...args]` — editor command invocation
* `"nu run" <name> [...args]` — Nu macro invocation (for chaining)
* `xeno ctx` — returns the current invocation context (same as `$env.XENO_CTX`, or `nothing` if not set)

Examples:

```nu
export def save-and-format [] {
  [(command write), (command format)]
}
export def move-down-5 [] {
  action move_down --count 5
}
export def context-aware [] {
  let ctx = (xeno ctx)
  if $ctx.mode == "Insert" {
    action normal_mode
  }
}
```

### Module-only load

`xeno.nu` is parsed and merged but **not evaluated** at load time. Only declarations are allowed at top level:

* `def` / `export def`
* `use` / `export use`
* `const` / `export const`
* `alias` / `export alias`
* `module` / `export module`

Top-level executable statements (expressions, function calls) are rejected.

### Hooks

Optional exported hook functions in `xeno.nu`:

* `on_action_post [name result]` — after an action completes
* `on_command_post [name result ...args]` — after a registry command completes
* `on_editor_command_post [name result ...args]` — after an editor command completes
* `on_mode_change [from to]` — after an actual mode transition (`from != to`)
* `on_buffer_open [path kind]` — after a buffer is opened or switched to

Result label values: `ok`, `quit`, `force_quit`, `not_found`, `cap_denied`, `readonly`, `error`.

Mode arguments are debug-formatted mode names (e.g. `"Normal"`, `"Insert"`, `"Prefix"`). `$env.XENO_CTX.mode` reflects the new mode at hook time.

Buffer open `kind` values: `"disk"` (loaded from filesystem), `"existing"` (switched to already-open document). `path` is always an absolute filesystem path. Hook fires on user navigation/focus changes (goto/open), not on internal buffer creation.

Hooks only run when the original result is non-quit. Hook-produced invocations are executed under a recursion guard (hooks cannot trigger more hooks). If a hook invocation returns `Quit` or `ForceQuit`, it propagates to the caller.

Hook functions use the same return schema as `nu-run` (typed invocations only) and are sandboxed with the same policy.

During `nu-run` and hook execution, Xeno sets `$env.XENO_CTX` with per-call context. Use `(xeno ctx)` to access it:

```nu
{
  schema_version: 1,
  kind: "macro" | "hook",
  function: "go",
  args: ["..."],
  mode: "Normal",
  view: { id: 1 },
  cursor: {
    line: 10,
    col: 4
  },
  selection: {
    active: true,
    start: { line: 10, col: 2 },
    end: { line: 10, col: 8 }
  },
  buffer: {
    path: "/abs/path/or/null",
    file_type: "rust" | null,
    readonly: false,
    modified: true
  }
}
```

Field semantics for `$env.XENO_CTX`:

* `cursor.line`, `cursor.col`, and `selection.start/end` coordinates are 0-based
* coordinates are character indices (not byte offsets)
* `selection.start` / `selection.end` are normalized bounds (`start <= end`)
* `selection.active == false` means a point selection (start and end equal the cursor position)

### Decode limits

Return values from macros and hooks are decoded with safety limits:

* max invocations: 256 (macros), 32 (hooks)
* max args per invocation: 64
* max string length: 4096
* max nodes visited: 50,000 (macros), 5,000 (hooks)

Exceeding any limit produces a descriptive error with the decode path (e.g. `return[2].args[0]`).

### Execution model

Macros and hooks run on a dedicated persistent worker thread (not the tokio blocking pool). Jobs are processed sequentially. If the worker panics, it exits cleanly and is auto-restarted on the next call with a single retry.

## Shared schema

Top-level fields:

- `options`: global option overrides
- `languages`: per-language option overrides
- `keys`: keymap overrides

### `options`

Record of option key to value.

Example:

```nu
{ options: { tab-width: 4, theme: "gruvbox" } }
```

### `languages`

List of records with:

* `name`: language name
* `options`: language-local option overrides

Example:

```nu
{
  languages: [
    { name: "rust", options: { tab-width: 2 } }
  ]
}
```

### `keys`

Record keyed by mode name, then key sequence to invocation target.

Supported target prefixes:

* `action:<action-id|action-name|action-key>` — execute a registry action
* `command:<name> [args...]` — execute a registry command
* `editor:<name> [args...]` — execute an editor command
* `nu:<fn> [args...]` — execute a Nu macro function from `xeno.nu`

Arguments may be quoted with `"..."` (supports `\"`, `\\`, `\n`, `\t`, `\r` escapes) or `'...'` (no escapes).

Binding values may be:

* string spec: `"command:write"`, `"editor:quit"`, `"nu:go fast"`
* record: `{ kind: "command", name: "write" }`
* custom value (`config.nu` only): `(command write)`

Example using string specs:

```nu
{
  keys: {
    normal: {
      "ctrl+s": "command:write",
      "ctrl+q": "editor:quit",
      "g r": "editor:reload_config",
      "ctrl+o": "command:open \"my file.txt\""
    }
  }
}
```

Record form (equivalent):

```nu
{
  keys: {
    normal: {
      "ctrl+s": { kind: "command", name: "write" },
      "ctrl+q": { kind: "editor", name: "quit" },
      "g r": { kind: "editor", name: "reload_config" },
      "ctrl+o": { kind: "command", name: "open", args: ["my file.txt"] }
    }
  }
}
```

## `config.nu` sandbox rules

`config.nu` runs in a restricted evaluator. The script is rejected when it attempts any of the following:

* external calls (`^cmd` or `run-external`)
* pipeline redirection
* `source`, `source-env`, or overlay loading commands (`overlay use/new/hide`)
* looping constructs (`for`, `while`, `loop`)
* `extern` / `export extern` declarations (external signatures)
* glob expressions (except `*` import selectors on `use`/`export use`)
* any parsed module file resolving outside the config directory root (including symlink escapes)

Nu's built-in operators (`ends-with`, `starts-with`, `like`, `=~`) work in the sandbox. Unknown commands may be treated as external calls and blocked.

`use` and `export use` path parsing and resolution are delegated to Nushell parser semantics, then Xeno enforces that every resolved module file remains under the config directory root.

* file modules and directory modules (`pkg/mod.nu`) are both supported
* nested module-relative imports are supported (for example `sub/a.nu` can `use b.nu *`)
* import selectors after the module path are allowed (for example: `use helper.nu *`)
* if a `use` is present, a real config directory root is required for validation

Depending on syntax and parse stage, failures can surface as either:

- `NuParse` when sandbox policy rejects the AST or when parsing/compilation fails

## Minimal examples

### `config.nu`

```nu
# config.nu — built-in commands are available (action, command, editor, "nu run", "xeno ctx")
{
  options: {
    tab-width: 4,
    theme: "gruvbox"
  },
  keys: {
    normal: {
      "ctrl+s": (command write),
      "ctrl+q": (editor quit),
      "g r": (editor reload_config),
    }
  }
}
```

### `config.nuon`

```nu
{
  options: { tab-width: 4 },
  languages: [
    { name: "rust", options: { tab-width: 2 } }
  ]
}
```

### `themes/*.nuon`

```nu
{
  name: "nuon-demo",
  variant: "dark",
  palette: {
    base: "#101010",
    fg: "#f0f0f0"
  },
  ui: {
    bg: "$base",
    fg: "$fg",
    nontext-bg: "#0a0a0a",
    gutter-fg: "gray",
    cursor-bg: "white",
    cursor-fg: "black",
    cursorline-bg: "#202020",
    selection-bg: "blue",
    selection-fg: "white",
    message-fg: "yellow",
    command-input-fg: "white"
  },
  mode: {
    normal-bg: "blue",
    normal-fg: "white",
    insert-bg: "green",
    insert-fg: "black",
    prefix-bg: "magenta",
    prefix-fg: "white",
    command-bg: "yellow",
    command-fg: "black"
  },
  semantic: {
    error: "red",
    warning: "yellow",
    success: "green",
    info: "cyan",
    hint: "dark-gray",
    dim: "dark-gray",
    link: "cyan",
    match: "green",
    accent: "cyan"
  },
  popup: {
    bg: "#111111",
    fg: "white",
    border: "white",
    title: "yellow"
  }
}
```
