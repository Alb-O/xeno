//! Notification definition macros.

/// Defines a notification emitter.
///
/// The metadata (level, auto-dismiss, etc.) comes from `notifications.kdl`.
/// This macro creates a `NotificationKey` typed handle and optionally a builder function.
#[macro_export]
macro_rules! notif {
	// Static message: notif!(name, "message")
	($name:ident, $msg:literal) => {
		paste::paste! {
			#[doc = concat!("Static notification handle for `", stringify!($name), "`.")]
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(
					concat!("xeno-registry::", stringify!($name)),
					$msg
				);
		}
	};

	// Parameterized: notif!(name(arg: Type, ...), format_expr)
	($name:ident ( $($arg:ident : $ty:ty),* $(,)? ), $fmt:expr) => {
		paste::paste! {
			/// Const key for pattern matching and introspection.
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(
					concat!("xeno-registry::", stringify!($name)),
					""
				);

			/// Builder function for parameterized notification.
			pub fn $name($($arg: $ty),*) -> $crate::notifications::Notification {
				$crate::notifications::Notification::new_pending(
					concat!("xeno-registry::", stringify!($name)),
					$fmt
				)
			}
		}
	};
}

/// Creates an alias key that references an existing notification definition.
#[macro_export]
macro_rules! notif_alias {
	($alias:ident, $base:ident, $msg:literal) => {
		paste::paste! {
			#[doc = concat!("Alias for ", stringify!($base), ": ", $msg)]
			pub const [<$alias:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(
					concat!("xeno-registry::", stringify!($base)),
					$msg
				);
		}
	};
}
