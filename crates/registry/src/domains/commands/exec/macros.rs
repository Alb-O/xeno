/// Registers a handler function for a registry-defined command.
///
/// The metadata (description, aliases, etc.) comes from `commands.nuon`; this macro
/// only provides the Rust handler and creates the inventory linkage.
#[macro_export]
macro_rules! command_handler {
	($name:ident, handler: $handler:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			pub(crate) static [<CMD_HANDLER_ $name>]: $crate::commands::CommandHandlerStatic =
				$crate::commands::CommandHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: $handler,
				};

			inventory::submit!($crate::commands::CommandHandlerReg(&[<CMD_HANDLER_ $name>]));
		}
	};
}
