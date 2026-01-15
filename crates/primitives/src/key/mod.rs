//! Key and input event types.
//!
//! This module provides a unified key representation that handles:
//! - Regular character keys
//! - Special keys (Escape, Enter, Tab, arrows, etc.)
//! - Modifier combinations (Ctrl, Alt, Shift)
//! - Mouse events (clicks, drags, scrolls)

mod keyboard;
mod modifiers;
mod mouse;

pub use keyboard::{Key, KeyCode};
pub use modifiers::Modifiers;
pub use mouse::{MouseButton, MouseEvent, ScrollDirection};
