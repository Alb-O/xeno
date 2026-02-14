//! Hook registration macros.
//!
//! `hook_handler!` for registering event lifecycle observers via NUON metadata.
//!
//! Note: The `__hook_extract!` and `__async_hook_extract!` macros are generated
//! by `xeno_macros::define_events!` in `lib.rs`.

/// Applies type-appropriate conversion for hook parameter extraction.
#[doc(hidden)]
#[macro_export]
macro_rules! __hook_param_expr {
	(Option<& $inner:ty>, $value:ident) => {
		$value.as_deref()
	};
	(Option < & $inner:ty >, $value:ident) => {
		$value.as_deref()
	};
	(& $inner:ty, $value:ident) => {
		&$value
	};
	(&$inner:ty, $value:ident) => {
		&$value
	};
	($ty:ty, $value:ident) => {
		$value
	};
}

/// Registers a handler for a registry-defined hook.
///
/// Metadata comes from `hooks.nuon`; this macro provides the handler function
/// and creates the inventory linkage.
#[macro_export]
macro_rules! hook_handler {
	($name:ident, $event:ident, |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hook_handler!($name, $event, |ctx| {
			__hook_extract!($event, ctx, $($param : $ty),*);
			$body
		});
	};
	($name:ident, $event:ident, |$ctx:ident| $body:expr) => {
		paste::paste! {
			fn [<hook_handler_ $name>]($ctx: &$crate::hooks::HookContext) -> $crate::hooks::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			pub(crate) static [<HOOK_HANDLER_ $name>]: $crate::hooks::handler::HookHandlerStatic =
				$crate::hooks::handler::HookHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: $crate::hooks::handler::HookHandlerConfig {
						event: $crate::HookEvent::$event,
						mutability: $crate::hooks::HookMutability::Immutable,
						execution_priority: $crate::hooks::HookPriority::Interactive,
						handler: $crate::hooks::HookHandler::Immutable([<hook_handler_ $name>]),
					},
				};

			inventory::submit!($crate::hooks::handler::HookHandlerReg(&[<HOOK_HANDLER_ $name>]));
		}
	};
}
