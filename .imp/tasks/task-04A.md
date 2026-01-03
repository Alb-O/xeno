# Xeno: Compile-Time Registry Verification - Architecture Review

## Summary

Analysis task that reviewed a build-script-based verification system and recommended a better approach.

## What Was Attempted

A compile-time verification system using:

- Build.rs scanning source files for macro invocations
- Generated `__motion_exists_*` constants
- `motion_ref!` macro that checked constants exist

## Problems Identified

1. **Scanner not robust** - fooled by doc comments, strings, edge cases
1. **Cross-crate limitation** - only verified items in same crate as constants
1. **Treats symptom not cause** - real problem is stringly-typed internal coupling
1. **Nightly dependency** - required `#![feature(macro_metavar_expr)]`

## Recommendation

Replace string-based internal references with typed handles:

```rust
// Before (string-based, error-prone)
cursor_motion(ctx, "left")

// After (typed, compile-time safe)
cursor_motion(ctx, motions::keys::left)
```

Key insights:

- Strings are fine at boundaries (user input, config)
- Internal code should use typed handles (`MotionKey`, `PanelKey`)
- Registration macros should generate both slice entries AND key constants
- No scanning needed - Rust name resolution provides safety

## Follow-up

See task-04B.md for implementation spec.
