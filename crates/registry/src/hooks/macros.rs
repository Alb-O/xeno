//! Hook registration macros.
//!
//! [`hook!`] and [`async_hook!`] for registering event lifecycle observers.
//!
//! Note: The `__hook_extract!` and `__async_hook_extract!` macros are generated
//! by `xeno_macro::define_events!` in `lib.rs`.

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

/// Define a hook.
#[macro_export]
macro_rules! hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, mutable |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(clippy::unused_unit)]
			fn [<hook_handler_ $name>](
				$ctx: &mut $crate::hooks::MutableHookContext,
			) -> $crate::hooks::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			pub static [<HOOK_ $name>]: $crate::hooks::HookDef = $crate::hooks::HookDef {
				meta: $crate::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: &[],
					description: $desc,
					priority: $priority,
					source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
					required_caps: &[],
					flags: 0,
				},
				event: $crate::HookEvent::$event,
				mutability: $crate::hooks::HookMutability::Mutable,
				execution_priority: $crate::hooks::HookPriority::Interactive,
				handler: $crate::hooks::HookHandler::Mutable([<hook_handler_ $name>]),
			};
		}
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hooks::hook!($name, $event, $priority, $desc, |ctx| {
			__hook_extract!($event, ctx, $($param : $ty),*);
			$body
		});
	};

	($name:ident, $event:ident, $priority:expr, $desc:expr, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(clippy::unused_unit)]
			fn [<hook_handler_ $name>]($ctx: &$crate::hooks::HookContext) -> $crate::hooks::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			pub static [<HOOK_ $name>]: $crate::hooks::HookDef = $crate::hooks::HookDef {
				meta: $crate::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: &[],
					description: $desc,
					priority: $priority,
					source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
					required_caps: &[],
					flags: 0,
				},
				event: $crate::HookEvent::$event,
				mutability: $crate::hooks::HookMutability::Immutable,
				execution_priority: $crate::hooks::HookPriority::Interactive,
				handler: $crate::hooks::HookHandler::Immutable([<hook_handler_ $name>]),
			};
		}
	};
}

/// Defines an async hook that owns extracted parameters.
#[macro_export]
macro_rules! async_hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, setup |$ctx:ident| { $($setup:tt)* } async || $body:expr) => {
		$crate::hooks::hook!($name, $event, $priority, $desc, |$ctx| {
			$($setup)*
			let owned = $ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				__async_hook_extract!($event, owned);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async || $body:expr) => {
		$crate::hooks::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				__async_hook_extract!($event, owned);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, setup |$ctx:ident| { $($setup:tt)* } async |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hooks::hook!($name, $event, $priority, $desc, |$ctx| {
			$($setup)*
			let owned = $ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hooks::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
}
