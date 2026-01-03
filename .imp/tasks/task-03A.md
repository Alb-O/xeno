# Xeno: Architectural Consistency Audit

## Model Directive

Perform a comprehensive architectural consistency audit of the Xeno editor codebase. Identify code smells, redundancies, inconsistent patterns, and structural issues that affect maintainability and clarity.

**Scope clarification**: This task focuses on *architectural shape* - module organization, abstraction boundaries, naming conventions, import patterns, and structural redundancies. This is NOT a security audit, correctness review, or bug hunt. Only flag safety/correctness issues if they are egregiously visible during the audit.

______________________________________________________________________

## Implementation Expectations

\<mandatory_execution_requirements>

This is an audit-then-fix task. The workflow is:

1. **Audit phase**: Systematically read through crates, noting issues in a structured format
1. **Report phase**: Present findings organized by severity and category
1. **Fix phase**: When directed, implement fixes using file editing tools
1. **Verify phase**: Run `cargo check --workspace` and `cargo test --workspace` after fixes

Do NOT:

- Silently fix issues without reporting them first
- Provide theoretical suggestions without concrete file/line references
- Focus on micro-optimizations or stylistic preferences that don't affect architecture
- Flag security/correctness issues unless they are severe and obvious

\</mandatory_execution_requirements>

______________________________________________________________________

## Behavioral Constraints

\<verbosity_and_scope_constraints>

- Report issues with concrete file paths and line numbers
- Categorize findings by type (redundancy, inconsistency, smell, etc.)
- Prioritize issues that affect multiple files or cross crate boundaries
- Skip trivial issues (single unused import, minor naming inconsistency)
- Do not rewrite working code just because you would write it differently

\</verbosity_and_scope_constraints>

\<design_freedom>

When fixing issues:

- Match existing patterns in the codebase
- Prefer minimal changes that address the root cause
- Consolidation and removal of code is preferred over adding new abstractions
- New patterns are acceptable only when they clearly simplify multiple locations

\</design_freedom>

______________________________________________________________________

## Audit Roadmap

### Phase 1: Crate Boundary Audit

Objective: Verify crate responsibilities are clear and non-overlapping.

**1.1 Review crate purposes**

For each crate in `crates/`, verify:

- Does the crate have a clear, singular purpose?
- Is the purpose documented in lib.rs or Cargo.toml?
- Are there types/functions that belong in a different crate?

Key crates to examine:

- `xeno-base` - Should contain only primitive types with minimal dependencies
- `xeno-core` - Glue layer; should not duplicate registry functionality
- `xeno-registry/*` - Each should be self-contained with clear ownership
- `xeno-api` - Editor engine; should not leak implementation details

**1.2 Check for boundary violations**

Look for:

- Circular dependency patterns (even if Cargo allows it via features)
- Types defined in one crate but primarily used in another
- Re-export chains longer than 2 hops
- Crates that exist only to re-export from others

Done: Document findings with file references

______________________________________________________________________

### Phase 2: Import Pattern Audit

Objective: Ensure consistent import patterns across the codebase.

**2.1 Catalog import styles**

Check for consistency in:

- `use crate::` vs `use super::` vs absolute paths
- Glob imports (`use foo::*`) - should be rare
- Re-export patterns (`pub use`)
- Import grouping and ordering

**2.2 Identify import smells**

Look for:

- Importing from re-export when direct import is clearer
- Unused imports (beyond what clippy catches)
- Overly deep import paths suggesting poor module structure
- Inconsistent paths to the same type across files

Done: Document patterns and inconsistencies

______________________________________________________________________

### Phase 3: Type Definition Audit

Objective: Ensure types are defined once and in appropriate locations.

**3.1 Find duplicate definitions**

Search for:

- Identical or near-identical structs/enums in different crates
- Types with the same name but different definitions
- Conversion boilerplate between equivalent types

**3.2 Check type placement**

Verify:

- Public types are in appropriate modules
- Internal types are not accidentally public
- Type aliases serve a real purpose (not just renaming)
- Generic types have appropriate bounds

Done: List duplicates and misplacements with locations

______________________________________________________________________

### Phase 4: Module Structure Audit

Objective: Ensure module hierarchy is logical and navigable.

**4.1 Review module depth**

Check for:

- Deeply nested modules (>3 levels) without clear justification
- Single-file modules that could be inlined
- Modules with only re-exports
- Inconsistent mod.rs vs filename.rs patterns

**4.2 Review module cohesion**

Look for:

- Modules mixing unrelated functionality
- Related functionality split across distant modules
- God modules with too many responsibilities
- Orphan modules with no clear parent concept

Done: Document structural issues

______________________________________________________________________

### Phase 5: Macro Audit

Objective: Ensure macros are necessary, well-placed, and consistently used.

**5.1 Catalog macros**

For each macro:

- Where is it defined?
- Where is it used?
- Could it be a function instead?
- Is it exported appropriately?

**5.2 Check macro patterns**

Look for:

- Macros that duplicate functionality
- Macros with inconsistent invocation patterns
- Proc macros that could be declarative (or vice versa)
- Macro hygiene issues

Done: List macro issues

______________________________________________________________________

### Phase 6: Registry Pattern Audit

Objective: Ensure all registry crates follow the established pattern.

**6.1 Compare registry crates**

Each `crates/registry/*` should have:

- `lib.rs` with types, slice, and lookup functions
- Consistent use of `RegistrySource` and `RegistryMetadata`
- Similar macro patterns for registration
- Parallel structure in `impls/` subdirectory

**6.2 Check registry consistency**

Look for:

- Registries that deviate from the pattern without reason
- Missing lookup functions or inconsistent signatures
- Inconsistent macro naming (`foo!` vs `register_foo!`)
- Slice naming inconsistencies

Done: Document deviations

______________________________________________________________________

## Issue Categories

Report findings using these categories:

| Category          | Description                                     | Severity    |
| ----------------- | ----------------------------------------------- | ----------- |
| **REDUNDANCY**    | Duplicate code, types, or logic                 | Medium-High |
| **BOUNDARY**      | Crate/module responsibility unclear or violated | High        |
| **INCONSISTENCY** | Pattern used differently across codebase        | Medium      |
| **SMELL**         | Code that works but suggests deeper issues      | Low-Medium  |
| **ORPHAN**        | Dead code, unused exports, vestigial modules    | Low         |
| **NAMING**        | Inconsistent or unclear naming patterns         | Low         |

______________________________________________________________________

## Reporting Format

For each finding, report:

```
## [CATEGORY] Short description

**Location**: `crate/path/to/file.rs:line`
**Also affects**: `other/file.rs`, `another/file.rs`

**Issue**: Concrete description of the problem.

**Suggestion**: Specific fix or investigation needed.
```

______________________________________________________________________

## Out of Scope

Explicitly do NOT audit:

- Test coverage or test quality
- Documentation completeness
- Performance characteristics
- Error handling patterns (unless architecturally significant)
- Unsafe code (unless obviously wrong)
- Clippy lints (assume CI catches these)
- Formatting (assume rustfmt is applied)

______________________________________________________________________

## Reference: Current Architecture

```
xeno-base          Primitives: Range, Selection, Key, Mode, Rope wrappers
xeno-registry/     Registry crates (actions, commands, hooks, panels, etc.)
xeno-core          Glue: ActionId, KeymapRegistry, movement, completion
xeno-api           Editor engine: Buffer, Editor, UI management
xeno-input         Input state machine
xeno-config        Configuration parsing (KDL)
xeno-language      Tree-sitter integration
xeno-lsp           LSP client framework
xeno-extensions    Host extensions (LSP, Zenmode)
xeno-tui           Ratatui fork
xeno-term          Main binary
```

Key patterns:

- Registry crates use `linkme` distributed slices
- `impl_registry_metadata!` macro for trait implementations
- `RegistrySource` and `Capability` defined in `xeno-registry-motions`
- Extensions discovered at build time via `build.rs`
