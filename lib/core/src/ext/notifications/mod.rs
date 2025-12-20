pub mod animation;
pub mod defaults;
pub mod layout;
pub mod manager;
pub mod notification;
pub mod render;
pub mod stacking;
pub mod state;
pub mod types;
pub mod ui;
pub mod utils;

use linkme::distributed_slice;
pub use manager::Notifications;
pub use notification::{Notification, NotificationBuilder, calculate_size, generate_code};
// Re-export ratatui types for convenience
pub use ratatui::layout::Position;
use ratatui::style::Style;
pub use types::{
	Anchor, Animation, AnimationPhase, AutoDismiss, Level, NotificationError, Overflow,
	SizeConstraint, SlideDirection, Timing,
};

#[distributed_slice]
pub static NOTIFICATION_TYPES: [NotificationTypeDef];

pub struct NotificationTypeDef {
	pub id: &'static str,
	pub name: &'static str,
	pub level: Level,
	pub icon: Option<&'static str>,
	pub style: Option<Style>,
	pub auto_dismiss: Option<AutoDismiss>,
	pub priority: i16,
	pub source: crate::ext::ExtensionSource,
}

pub fn find_notification_type(name: &str) -> Option<&'static NotificationTypeDef> {
	NOTIFICATION_TYPES.iter().find(|t| t.name == name)
}

#[macro_export]
macro_rules! notification_type {
	($static_name:ident, $name:literal, $level:expr, $icon:expr, $style:expr, $auto_dismiss:expr) => {
		#[::linkme::distributed_slice($crate::ext::notifications::NOTIFICATION_TYPES)]
		static $static_name: $crate::ext::notifications::NotificationTypeDef =
			$crate::ext::notifications::NotificationTypeDef {
				id: $name,
				name: $name,
				level: $level,
				icon: $icon,
				style: $style,
				auto_dismiss: $auto_dismiss,
				priority: 0,
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
			};
	};
}
