# Xeno config formats

Xeno supports three user config entrypoints, loaded in this order:

1. `~/.config/xeno/config.kdl`
2. `~/.config/xeno/config.nuon`
3. `~/.config/xeno/config.nu`

Later files win when fields overlap.

Options and keys merge with later layers overriding earlier values. Language entries are accumulated, and later entries for the same language take precedence when applied.

## Precedence

| Layer | File | Precedence |
| --- | --- | --- |
| 1 | `config.kdl` | lowest |
| 2 | `config.nuon` | overrides KDL |
| 3 | `config.nu` | highest |

`config.nu` must evaluate to a record that follows the same schema as `config.nuon`.

## Runtime reload

Use `:reload-config` (or `:reload_config`) to reload config files without restarting the editor.

- re-reads `config.kdl`, `config.nuon`, and `config.nu` in precedence order
- replaces current key overrides and option layers with the newly loaded config state
- logs per-file warnings and errors, then refreshes theme loading

## Shared schema

Top-level fields:

- `options`: global option overrides
- `languages`: per-language option overrides
- `keys`: keymap overrides
- `theme`: reserved (currently not used for inline theme objects)

### `options`

Record of option key to value.

Example:

```nu
{ options: { tab-width: 4, theme: "gruvbox" } }
```

### `languages`

List of records with:

- `name`: language name
- `options`: language-local option overrides

Example:

```nu
{
  languages: [
    { name: "rust", options: { tab-width: 2 } }
  ]
}
```

### `keys`

Record keyed by mode name, then key sequence to action target.

Current target support:

- `action:<action-id|action-name|action-key>`

Targets are resolved against the actions registry.

`command:*` targets are currently ignored by the key override resolver.

Example:

```nu
{
  keys: {
    normal: {
      "ctrl+s": "action:xeno-registry::some_action_id"
    }
  }
}
```

## `config.nu` sandbox rules

`config.nu` runs in a restricted evaluator. The script is rejected when it attempts any of the following:

- external calls
- pipeline redirection
- `source` and overlay loading commands
- `use`/`export use` paths outside the config directory root
- looping constructs (`for`, `while`, `loop`)
- glob expressions
- blocked process, filesystem, network, or plugin command names

`use` and `export use` are allowed only for static `.nu` paths rooted under the directory containing `config.nu` (the Xeno config directory).

- path must be a static literal (no interpolation)
- path must be relative and cannot contain `..`
- path must not contain glob wildcard characters
- resolved canonical target must stay under the config root and point to a file

Depending on syntax and parse stage, failures can surface as either:

- `NuSandbox` when sandbox policy rejected valid AST
- `NuParse` when parsing/compilation fails before sandbox traversal can continue

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
