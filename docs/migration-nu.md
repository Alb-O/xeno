# Nu scripting migration guide

This documents breaking changes to the Nu scripting integration.

## Prelude removal

The built-in `xeno` prelude module has been removed. If your `xeno.nu` contains `use xeno *`, delete that line. The built-in commands are registered directly and available without any import.

Before:

```nu
use xeno *
export def go [] { action move_right }
```

After:

```nu
export def go [] { action move_right }
```

Removed prelude helpers: `default`, `is-null`, `str ends-with`, `str starts-with`, `str contains`, `$XENO_PRELUDE_VERSION`. Use Nu's built-in operators (`ends-with`, `starts-with`, `=~`) or define your own helpers.

## Runtime returns are typed-only

Macros and hooks must return values from built-in commands. Records and strings are no longer accepted as runtime return values.

Before:

```nu
export def go [] {
  { kind: "action", name: "move_right", count: 2 }
}
export def multi [] {
  [
    { kind: "editor", name: "stats" },
    { kind: "command", name: "help" }
  ]
}
```

After:

```nu
export def go [] {
  action move_right --count 2
}
export def multi [] {
  [(editor stats), (command help)]
}
```

## Context access

Use `(xeno ctx)` instead of `$env.XENO_CTX` (both work, but the command is the recommended API):

```nu
export def context-aware [] {
  let ctx = (xeno ctx)
  if $ctx.mode == "Insert" {
    action normal_mode
  }
}
```

## Keybindings

Config keybindings now accept string specs in addition to records:

```nu
{
  keys: {
    normal: {
      "ctrl+s": "command:write",
      "ctrl+q": "editor:quit",
      "ctrl+o": "command:open \"my file.txt\""
    }
  }
}
```

## Decode limits

The `max_depth` config field has been removed. Remaining configurable limits: `max_invocations`, `max_string_len`, `max_args`, `max_action_count`, `max_nodes`.

## Built-in commands reference

* `action <name> [--count N] [--extend] [--register R] [--char C]`
* `command <name> [...args]`
* `editor <name> [...args]`
* `"nu run" <name> [...args]`
* `xeno ctx` — returns the current invocation context or nothing
