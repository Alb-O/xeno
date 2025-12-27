//! Notification rendering system.
//!
//! This module handles the UI rendering of notifications. It converts
//! abstract `Notification` data from `tome_stdlib` into rendered widgets
//! using tome_tui.
//!
//! # Architecture
//!
//! - **Manager** (`manager.rs`): Manages notification lifecycle and delegates rendering.
//! - **State** (`state.rs`): Runtime state with tome_tui types.
//! - **Render** (`render.rs`): Main rendering logic.
//! - **Layout** (`layout.rs`): Position calculation.
//! - **Stacking** (`stacking.rs`): Multiple notification stacking.
//! - **Animation** (`animation/`): Animation effects.
//! - **UI** (`ui.rs`): UI helper functions.
//! - **Utils** (`utils.rs`): Utility functions.

mod animation;
mod layout;
mod manager;
mod notification;
mod render;
mod stacking;
mod state;
mod types;
mod ui;
mod utils;

pub use manager::Notifications;
pub use tome_manifest::notifications::Overflow;
