pub(crate) mod classes;
pub mod functions;
pub mod orc_manager;
pub mod orc_render;
pub mod orc_stacking;
pub mod types;

// Re-export main types for convenient access
pub use classes::{Notification, NotificationBuilder};
// Re-export layout utilities for custom positioning
pub use functions::fnc_calculate_anchor_position::calculate_anchor_position;
pub use functions::fnc_calculate_rect::calculate_rect;
pub use functions::fnc_calculate_size::calculate_size;
// Re-export code generation utility
pub use functions::fnc_generate_code::generate_code;
pub use orc_manager::Notifications;
pub use types::{
	Anchor, Animation, AnimationPhase, AutoDismiss, Level, NotificationError, Overflow,
	SizeConstraint, SlideDirection, Timing,
};
