//! Notification definition macros.
//!
//! See [`notif!`] for defining notifications and [`notif_alias!`] for creating
//! additional keys that reference an existing notification definition.

/// Defines a notification with compile-time registration.
///
/// # Static Notifications
///
/// ```ignore
/// notif!(buffer_readonly, Warn, "Buffer is read-only");
/// // Generates: BUFFER_READONLY const
/// // Usage: ctx.emit(keys::BUFFER_READONLY);
/// ```
///
/// # Parameterized Notifications
///
/// ```ignore
/// notif!(yanked_chars(count: usize), Info, format!("Yanked {} chars", count));
/// // Generates: YANKED_CHARS const + yanked_chars() function
/// // Usage: ctx.emit(keys::yanked_chars(42));
/// ```
///
/// # Custom Auto-Dismiss
///
/// ```ignore
/// notif!(regex_error(err: &str), Error, format!("Regex error: {}", err),
///        auto_dismiss: AutoDismiss::After(Duration::from_secs(8)));
/// ```
#[macro_export]
macro_rules! notif {
	// Static message: notif!(name, Level, "message")
	($name:ident, $level:ident, $msg:literal) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::NotificationDef = $crate::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::Level::$level,
				$crate::AutoDismiss::DEFAULT,
				$crate::RegistrySource::Builtin,
			);
			inventory::submit! { $crate::NotificationReg(&[<NOTIF_ $name:upper>]) }

			#[doc = concat!("Static notification: ", $msg)]
			pub const [<$name:upper>]: $crate::NotificationKey =
				$crate::NotificationKey::new(&[<NOTIF_ $name:upper>], $msg);
		}
	};

	// Static message with custom auto_dismiss: notif!(name, Level, "message", auto_dismiss: expr)
	($name:ident, $level:ident, $msg:literal, auto_dismiss: $dismiss:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::NotificationDef = $crate::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::Level::$level,
				$dismiss,
				$crate::RegistrySource::Builtin,
			);
			inventory::submit! { $crate::NotificationReg(&[<NOTIF_ $name:upper>]) }

			#[doc = concat!("Static notification: ", $msg)]
			pub const [<$name:upper>]: $crate::NotificationKey =
				$crate::NotificationKey::new(&[<NOTIF_ $name:upper>], $msg);
		}
	};

	// Parameterized: notif!(name(arg: Type, ...), Level, format_expr)
	($name:ident ( $($arg:ident : $ty:ty),* $(,)? ), $level:ident, $fmt:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::NotificationDef = $crate::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::Level::$level,
				$crate::AutoDismiss::DEFAULT,
				$crate::RegistrySource::Builtin,
			);
			inventory::submit! { $crate::NotificationReg(&[<NOTIF_ $name:upper>]) }

			/// Const key for pattern matching and introspection.
			pub const [<$name:upper>]: $crate::NotificationKey =
				$crate::NotificationKey::new(&[<NOTIF_ $name:upper>], "");

			/// Builder function for parameterized notification.
			pub fn $name($($arg: $ty),*) -> $crate::Notification {
				$crate::Notification::new(&[<NOTIF_ $name:upper>], $fmt)
			}
		}
	};

	// Parameterized with custom auto_dismiss: notif!(name(arg: Type, ...), Level, format_expr, auto_dismiss: expr)
	($name:ident ( $($arg:ident : $ty:ty),* $(,)? ), $level:ident, $fmt:expr, auto_dismiss: $dismiss:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::NotificationDef = $crate::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::Level::$level,
				$dismiss,
				$crate::RegistrySource::Builtin,
			);
			inventory::submit! { $crate::NotificationReg(&[<NOTIF_ $name:upper>]) }

			/// Const key for pattern matching and introspection.
			pub const [<$name:upper>]: $crate::NotificationKey =
				$crate::NotificationKey::new(&[<NOTIF_ $name:upper>], "");

			/// Builder function for parameterized notification.
			pub fn $name($($arg: $ty),*) -> $crate::Notification {
				$crate::Notification::new(&[<NOTIF_ $name:upper>], $fmt)
			}
		}
	};
}

/// Creates an alias key that references an existing notification definition.
///
/// Use this when you need multiple keys with different messages that share
/// the same notification ID (for filtering/matching purposes).
///
/// # Example
///
/// ```ignore
/// notif!(no_selection, Warn, "No selection");
/// notif_alias!(no_selection_to_search, no_selection, "No selection to search in");
/// notif_alias!(no_selection_to_split, no_selection, "No selection to split");
/// ```
#[macro_export]
macro_rules! notif_alias {
	($alias:ident, $base:ident, $msg:literal) => {
		paste::paste! {
			#[doc = concat!("Alias for ", stringify!($base), ": ", $msg)]
			pub const [<$alias:upper>]: $crate::NotificationKey =
				$crate::NotificationKey::new(&[<NOTIF_ $base:upper>], $msg);
		}
	};
}
