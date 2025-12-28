# Linting

Custom [ast-grep](https://ast-grep.github.io/) rules catch trivial comments and other patterns that clutter the codebase.

Run `lint` from the nix dev shell to see all violations grouped by rule:

```bash
nix develop
lint
```

The output consolidates violations by type, showing each occurrence with its actual comment text rather than dumping individual reports. This makes batch fixes obvious: you can see that twelve `// Insert mode` headers are decorating keybinding lists that already say "insert" in their variable names.

```
INFO: no-short-comments - Short single-line comments may not be necessary. Code should be self-explanatory.
NOTE: Consider moving important context to function docstrings with examples, or removing if truly trivial.
  - crates/evildoer-core/src/input.rs:631:13 - // Fallback to lowercase
  - crates/evildoer-macro/src/lib.rs:29:73 - // skip self
  - crates/evildoer-core/src/ext/actions/scroll.rs:91:39 - // TODO: Needs viewport info
```

The pre-commit hook runs this same scan and blocks commits when violations exist. Fix them or the commit fails.

If you need ast-grep's full source context for a specific violation, `ast-grep scan` still works. The consolidated view is just easier for cleaning up patterns.

## Short comments

The `no-short-comments` rule flags single-line comments under 25 characters. These usually just narrate code that already speaks for itself. A comment reading `// Search backward` above a loop that iterates `(0..pos).rev()` wastes a line. The loop already says "backward".

When the comment explains something non-obvious, move it to a docstring with examples. The function signature then documents the behavior where readers actually look for it, rather than hiding the explanation inline where it fragments the implementation.

Comments like `// hex prefix` next to `ch == 'x' || ch == 'X'` might seem helpful until you realize the surrounding function processes number literals across multiple bases. A proper docstring listing all supported formats (decimal, hex with `0x`, binary with `0b`, octal with `0o`, scientific notation with `e`) beats five inline comments that each explain one character check.

Keep comments that document workarounds, gotchas, or algorithm steps that aren't self-evident from the code structure. `// Wrap around` in a search function tells you the behavior continues from the start when it hits the end, which matters. `// Find character` above `find_char_forward()` wastes everyone's time.
