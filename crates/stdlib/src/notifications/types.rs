//! Notification types re-exported from evildoer-registry.
//!
//! This module provides convenient access to notification-related types.
//! All types are defined in `evildoer_registry::notifications` to keep them
//! UI-agnostic.

pub use evildoer_registry::notifications::{
	Anchor, Animation, AnimationPhase, AutoDismiss, Level, NotificationError, Overflow,
	SizeConstraint, SlideDirection, Timing,
};
