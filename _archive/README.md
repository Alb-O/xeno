# Archive

Shelved features preserved for potential future use. These are **not** part of the
workspace and will not compile.

## auth/

OAuth/PKCE authentication providers for AI services (Claude, Codex). Includes:
- Token storage and refresh
- Local callback server for OAuth flow
- XDG-compliant credential storage

**Why archived:** AI integration deferred; editor core prioritized.

## zenmode/

Focus mode that dims syntax highlighting outside the current tree-sitter element.
Uses the style overlay system to create a spotlight effect on the code block
containing the cursor.

**Why archived:** Style overlay API not yet stabilized.
