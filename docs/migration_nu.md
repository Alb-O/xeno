# Nu scripting migration guide

This documents breaking changes to the Nu scripting integration.

## Prelude removal

The built-in `xeno` prelude module has been removed. If your `xeno.nu` contains `use xeno *`, delete that line.

Before:

```nu
use xeno *
export def go [] { xeno emit action move_right }
```

After:

```nu
export def go [] { xeno emit action move_right }
```

## Constructor command reset

The old constructor commands are removed:

* `action`
* `command`
* `editor`
* `nu run`

Use the v2 commands instead:

* `xeno emit <kind> <name> ...`
* `xeno emit-many`
* `xeno call <name> ...`

Before:

```nu
export def go [] {
  action move_right --count 2
}
export def multi [] {
  [(editor stats), (command help)]
}
export def chain [] {
  nu run other a b
}
```

After:

```nu
export def go [] {
  xeno emit action move_right --count 2
}
export def multi [] {
  [(xeno emit editor stats), (xeno emit command help)]
}
export def chain [] {
  xeno call other a b
}
```

## Runtime return contract

Macros and hooks must return one of:

* `null` / nothing
* one valid invocation record
* a list of valid invocation records and/or nothing values

String returns are rejected.

## Context access

Use `(xeno ctx)` for invocation context:

```nu
export def context-aware [] {
  let ctx = (xeno ctx)
  if $ctx.mode == "Insert" {
    xeno emit action normal_mode
  }
}
```

## Keybindings

String key targets remain unchanged:

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

If you used `config.nu` custom values, switch to `xeno emit` forms.

Before:

```nu
{ keys: { normal: { "ctrl+s": (command write) } } }
```

After:

```nu
{ keys: { normal: { "ctrl+s": (xeno emit command write) } } }
```

## Decode limits

`max_depth` is removed. Remaining configurable limits:

* `max_invocations`
* `max_string_len`
* `max_args`
* `max_action_count`
* `max_nodes`

## Built-in commands reference

* `xeno emit <kind> <name> [...args] [--count N] [--extend] [--register R] [--char C]`
* `xeno emit-many`
* `xeno call <name> [...args]`
* `xeno ctx`
* `xeno assert`
* `xeno is-invocation`
* `xeno log`
