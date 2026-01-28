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
			pub static [<CMD_ $name>]: $crate::commands::CommandDef = $crate::commands::CommandDef {
				meta: $crate::commands::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: $crate::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: $crate::__reg_opt!($({$priority})?, 0),
					source: $crate::__reg_opt!(
						$({$source})?,
						$crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))
					),
					required_caps: $crate::__reg_opt_slice!($({$caps})?),
					flags: $crate::__reg_opt!($({$flags})?, $crate::commands::flags::NONE),
				},
				handler: $handler,
				user_data: None,
			};
		}
	};
}
