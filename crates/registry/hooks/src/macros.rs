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

/// Define a hook and register it in the [`HOOKS`](crate::HOOKS) slice.
///
/// # Example
///
/// ```ignore
/// hook!(log_open, BufferOpen, 100, "Log buffer opens", |ctx| {
///     if let HookContext::BufferOpen { path, .. } = ctx {
///         tracing::info!(path = %path.display(), "Opened buffer");
///     }
/// });
/// ```
#[macro_export]
macro_rules! hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, mutable |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(clippy::unused_unit)]
			fn [<hook_handler_ $name>](
				$ctx: &mut $crate::MutableHookContext,
			) -> $crate::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::HOOKS)]
			static [<HOOK_ $name>]: $crate::HookDef = $crate::HookDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				event: $crate::HookEvent::$event,
				description: $desc,
				priority: $priority,
				mutability: $crate::HookMutability::Mutable,
				handler: $crate::HookHandler::Mutable([<hook_handler_ $name>]),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |ctx| {
			$crate::__hook_extract!($event, ctx, $($param : $ty),*);
			$body
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(clippy::unused_unit)]
			fn [<hook_handler_ $name>]($ctx: &$crate::HookContext) -> $crate::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::HOOKS)]
			static [<HOOK_ $name>]: $crate::HookDef = $crate::HookDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				event: $crate::HookEvent::$event,
				description: $desc,
				priority: $priority,
				mutability: $crate::HookMutability::Immutable,
				handler: $crate::HookHandler::Immutable([<hook_handler_ $name>]),
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Defines an async hook that owns extracted parameters.
#[macro_export]
macro_rules! async_hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, setup |$ctx:ident| { $($setup:tt)* } async || $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |$ctx| {
			$($setup)*
			let owned = $ctx.to_owned();
			$crate::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async || $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, setup |$ctx:ident| { $($setup:tt)* } async |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |$ctx| {
			$($setup)*
			let owned = $ctx.to_owned();
			$crate::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
}
