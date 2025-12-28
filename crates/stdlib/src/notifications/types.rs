//! Notification types re-exported from evildoer-manifest.
//!
//! This module provides convenient access to notification-related types.
//! All types are defined in `evildoer_manifest::notifications` to keep them
//! UI-agnostic.

pub use evildoer_manifest::notifications::{
	Anchor, Animation, AnimationPhase, AutoDismiss, Level, NotificationError, Overflow,
	SizeConstraint, SlideDirection, Timing,
};
