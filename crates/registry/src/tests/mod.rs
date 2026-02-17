//! Registry integration tests for consistency and keymap behavior.

mod consistency;
#[cfg(all(test, feature = "minimal", feature = "actions", feature = "keymap"))]
mod reactive_keymap;
