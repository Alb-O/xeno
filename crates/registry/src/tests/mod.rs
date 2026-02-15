//! Registry integration tests for consistency and keymap behavior.

mod consistency;
#[cfg(all(test, feature = "db", feature = "actions", feature = "keymap"))]
mod reactive_keymap;
