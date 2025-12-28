# Zen Mode Extension

A focus mode that dims syntax highlighting outside the current tree-sitter
element (function, struct, impl, etc.) containing the cursor.

## Usage

Toggle zen mode with one of these commands:

- `:zen`
- `:zenmode`
- `:focus`

When enabled, text outside the current code block will be dimmed, helping you
focus on the code you're actively editing.

## How it works

On each render frame, the extension:

1. Finds the tree-sitter node at the cursor position
1. Walks up the syntax tree to find the best container node
1. Registers a style overlay that dims everything outside that container

### Focus Node Selection

The extension uses a two-tier priority system:

**Primary Focus Nodes** (highest priority):

- Functions: `function_item`, `function_definition`, `closure_expression`, etc.
- Types: `struct_item`, `enum_item`, `trait_item`, `impl_item`, etc.
- Modules: `mod_item`, `module`, `namespace_definition`

**Secondary Focus Nodes** (fallback):

- Top-level items: `const_item`, `static_item`, `type_alias`, `use_declaration`
- Control flow: `match_expression`, `if_expression`, `for_expression`, etc.
- Blocks: `block`, `statement_block`

When the cursor is inside a string literal or other nested expression, the
extension walks up the tree until it finds a primary focus node. If none is
found, it falls back to secondary focus nodes.

## Configuration

The default dim factor is 0.35 (35% brightness toward background). This can be
adjusted in the extension state if needed.

## Architecture

This extension uses the generic **Style Overlay** system in evildoer-api.
Extensions can register style modifications (dimming, color overrides) for byte
ranges, and the renderer applies them during text rendering.

Key components:

- `RENDER_EXTENSIONS`: Distributed slice for render-time updates
- `StyleOverlays`: Collection of style modifications on the Editor
- `dim_outside()`: Helper to dim everything outside a focus range

This approach:

- Keeps the extension decoupled from the renderer
- Allows multiple extensions to apply style modifications
- Uses tree-sitter for semantic code understanding
- Updates correctly after mouse clicks (uses render-time hooks)
