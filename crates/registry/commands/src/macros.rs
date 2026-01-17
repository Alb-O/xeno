/// Selects a provided value or falls back to a default.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Selects a provided slice or returns an empty slice.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt_slice {
	({$val:expr}) => {
		$val
	};
	() => {
		&[]
	};
}

/// Defines an ex-mode command.
#[macro_export]
macro_rules! command {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub static [<CMD_ $name>]: $crate::CommandDef = $crate::CommandDef {
				meta: $crate::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: $crate::__opt_slice!($({$aliases})?),
					description: $desc,
					priority: $crate::__opt!($({$priority})?, 0),
					source: $crate::__opt!(
						$({$source})?,
						$crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))
					),
					required_caps: $crate::__opt_slice!($({$caps})?),
					flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
				},
				handler: $handler,
				user_data: None,
			};

			inventory::submit! { $crate::CommandReg(&[<CMD_ $name>]) }
		}
	};
}
