//! Notification toast widget for displaying transient messages.
//!
//! This module provides a self-contained notification system with:
//! - Configurable anchor positions (corners and edges)
//! - Multiple animation styles (slide, fade, expand/collapse)
//! - Automatic stacking of multiple notifications
//! - Auto-dismiss with configurable timing
//!
//! # Example
//!
//! ```ignore
//! use std::time::Duration;
//! use evildoer_tui::widgets::notifications::{Toast, ToastManager, Anchor, Level};
//!
//! let mut manager = ToastManager::new();
//! manager.push(
//!     Toast::new("File saved successfully")
//!         .level(Level::Info)
//!         .anchor(Anchor::BottomRight)
//! );
//!
//! // In your render loop:
//! manager.tick(Duration::from_millis(16));
//! manager.render(frame.area(), frame.buffer_mut());
//! ```

mod manager;
mod toast;
mod types;

pub use manager::ToastManager;
pub use toast::{Toast, ToastIcon, ICON_COLUMN_WIDTH};
pub use types::{
	Anchor, Animation, AnimationPhase, AutoDismiss, Level, Overflow, SizeConstraint,
	SlideDirection, Timing,
};
