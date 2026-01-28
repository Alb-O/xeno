//! Notification definition macros.

/// Defines a notification with compile-time registration.
#[macro_export]
macro_rules! notif {
	// Static message: notif!(name, Level, "message")
	($name:ident, $level:ident, $msg:literal) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::notifications::NotificationDef = $crate::notifications::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::notifications::Level::$level,
				$crate::notifications::AutoDismiss::DEFAULT,
				$crate::RegistrySource::Builtin,
			);

			#[doc = concat!("Static notification: ", $msg)]
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(&[<NOTIF_ $name:upper>], $msg);
		}
	};

	// Static message with custom auto_dismiss: notif!(name, Level, "message", auto_dismiss: expr)
	($name:ident, $level:ident, $msg:literal, auto_dismiss: $dismiss:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::notifications::NotificationDef = $crate::notifications::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::notifications::Level::$level,
				$dismiss,
				$crate::RegistrySource::Builtin,
			);

			#[doc = concat!("Static notification: ", $msg)]
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(&[<NOTIF_ $name:upper>], $msg);
		}
	};

	// Parameterized: notif!(name(arg: Type, ...), Level, format_expr)
	($name:ident ( $($arg:ident : $ty:ty),* $(,)? ), $level:ident, $fmt:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::notifications::NotificationDef = $crate::notifications::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::notifications::Level::$level,
				$crate::notifications::AutoDismiss::DEFAULT,
				$crate::RegistrySource::Builtin,
			);

			/// Const key for pattern matching and introspection.
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(&[<NOTIF_ $name:upper>], "");

			/// Builder function for parameterized notification.
			pub fn $name($($arg: $ty),*) -> $crate::notifications::Notification {
				$crate::notifications::Notification::new(&[<NOTIF_ $name:upper>], $fmt)
			}
		}
	};

	// Parameterized with custom auto_dismiss: notif!(name(arg: Type, ...), Level, format_expr, auto_dismiss: expr)
	($name:ident ( $($arg:ident : $ty:ty),* $(,)? ), $level:ident, $fmt:expr, auto_dismiss: $dismiss:expr) => {
		paste::paste! {
			static [<NOTIF_ $name:upper>]: $crate::notifications::NotificationDef = $crate::notifications::NotificationDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				$crate::notifications::Level::$level,
				$dismiss,
				$crate::RegistrySource::Builtin,
			);

			/// Const key for pattern matching and introspection.
			pub const [<$name:upper>]: $crate::notifications::NotificationKey =
				$crate::notifications::NotificationKey::new(&[<NOTIF_ $name:upper>], "");

			/// Builder function for parameterized notification.
			pub fn $name($($arg: $ty),*) -> $crate::notifications::Notification {
				$crate::notifications::Notification::new(&[<NOTIF_ $name:upper>], $fmt)
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
				$crate::notifications::NotificationKey::new(&[<NOTIF_ $base:upper>], $msg);
		}
	};
}
