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

* string: single invocation
* list of strings: multiple invocations
* record: `{ invocations: ["..."] }`

Supported invocation prefixes:

* `action:<id|name|key>`
* `command:<name> [args...]`
* `editor:<name> [args...]`
* `nu:<fn> [args...]`

Structured invocation records are also supported:

```nu
{ kind: "action", name: "move_right", count: 2 }
{ kind: "action", name: "find_char", char: "x" }
{ kind: "command", name: "help", args: ["themes"] }
{ kind: "editor", name: "reload_config", args: [] }
{ kind: "nu", name: "go", args: ["fast"] }
```

You can return a single record or a list of records:

```nu
[
  { kind: "editor", name: "stats" },
  { kind: "action", name: "move_right", count: 2 }
]
```

Optional exported hook functions in `xeno.nu`:

* `on_action_post [action_name result]`
* `on_mode_change [old_mode new_mode]`

Hook functions use the same return schema as `nu-run` and are sandboxed with the same policy as `config.nu` and `xeno.nu` macros.

During `nu-run` and hook execution, Xeno sets `$env.XENO_CTX` with per-call context:

```nu
{
  kind: "macro" | "hook",
  function: "go",
  args: ["..."],
  mode: "Normal",
  buffer: {
    path: "/abs/path/or/null",
    file_type: "rust" | null,
    readonly: false
  }
}
```

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

Example:

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

## `config.nu` sandbox rules

`config.nu` runs in a restricted evaluator. The script is rejected when it attempts any of the following:

* external calls
* pipeline redirection
* `source` and overlay loading commands
* `use`/`export use` paths outside the config directory root
* looping constructs (`for`, `while`, `loop`)
* glob expressions
* blocked process, filesystem, network, or plugin command names

`use` and `export use` are allowed only for static `.nu` paths rooted under the directory containing `config.nu` (the Xeno config directory).

* path must be a static literal (no interpolation)
* path must be relative and cannot contain `..`
* path must not contain glob wildcard characters
* resolved canonical target must stay under the config root and point to a file

Depending on syntax and parse stage, failures can surface as either:

- `NuParse` when sandbox policy rejects the AST or when parsing/compilation fails

## Minimal examples

### `config.nu`

```nu
{
  options: {
    tab-width: 4,
    theme: "gruvbox"
  },
  keys: {
    normal: {
      "ctrl+s": "action:xeno-registry::some_action_id"
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
