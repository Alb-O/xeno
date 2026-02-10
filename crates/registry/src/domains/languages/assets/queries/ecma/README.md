# Inheritance model for ECMA-family languages

ECMA-family grammars share large portions of query logic. We keep inheritance
predictable by splitting shared query fragments into "private" query
directories prefixed with `_` and reserving public directories for user-facing
language names.

- Private layers should not declare `; inherits`.
- Public layers compose private layers (and `ecma`) explicitly.
- Add shared rules to the most specific private layer so downstream languages
  inherit them automatically.

| Language | Inherits from |
| --- | --- |
| `javascript` | `_javascript`, `ecma` |
| `jsx` | `_jsx`, `_javascript`, `ecma` |
| `typescript` | `_typescript`, `ecma` |
| `tsx` | `_jsx`, `_typescript`, `ecma` |
| `gjs` | `_gjs`, `_javascript`, `ecma` |
| `gts` | `_gjs`, `_typescript`, `ecma` |

When adding or adjusting ECMA-family queries, update the relevant private layer
first, then keep each public layer focused on language-specific deltas.
