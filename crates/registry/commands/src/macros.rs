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
					aliases: xeno_registry_core::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: xeno_registry_core::__reg_opt!($({$priority})?, 0),
					source: xeno_registry_core::__reg_opt!(
						$({$source})?,
						$crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))
					),
					required_caps: xeno_registry_core::__reg_opt_slice!($({$caps})?),
					flags: xeno_registry_core::__reg_opt!($({$flags})?, $crate::flags::NONE),
				},
				handler: $handler,
				user_data: None,
			};

			inventory::submit! { $crate::CommandReg(&[<CMD_ $name>]) }
		}
	};
}
