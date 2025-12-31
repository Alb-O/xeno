//! Hook registration macros.
//!
//! [`hook!`] and [`async_hook!`] for registering event lifecycle observers.

#[doc(hidden)]
#[macro_export]
macro_rules! __hook_extract {
	(EditorStart, $ctx:ident $(,)?) => {
		let $crate::hooks::HookEventData::EditorStart = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
	};
	(EditorQuit, $ctx:ident $(,)?) => {
		let $crate::hooks::HookEventData::EditorQuit = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
	};
	(EditorTick, $ctx:ident $(,)?) => {
		let $crate::hooks::HookEventData::EditorTick = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
	};
	(FocusGained, $ctx:ident $(,)?) => {
		let $crate::hooks::HookEventData::FocusGained = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
	};
	(FocusLost, $ctx:ident $(,)?) => {
		let $crate::hooks::HookEventData::FocusLost = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
	};
	(BufferOpen, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::BufferOpen { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(BufferWritePre, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::BufferWritePre { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(BufferWrite, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::BufferWrite { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(BufferClose, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::BufferClose { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(BufferChange, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::BufferChange { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(ModeChange, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::ModeChange { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(CursorMove, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::CursorMove { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(SelectionChange, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::SelectionChange { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
	(WindowResize, $ctx:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::HookEventData::WindowResize { $($param,)* .. } = &$ctx.data else {
			return $crate::hooks::HookAction::Done($crate::hooks::HookResult::Continue);
		};
		$(let $param: $ty = $param; )*
	};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __async_hook_extract {
	(EditorStart, $owned:ident $(,)?) => {
		let $crate::hooks::OwnedHookContext::EditorStart = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
	};
	(EditorQuit, $owned:ident $(,)?) => {
		let $crate::hooks::OwnedHookContext::EditorQuit = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
	};
	(EditorTick, $owned:ident $(,)?) => {
		let $crate::hooks::OwnedHookContext::EditorTick = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
	};
	(FocusGained, $owned:ident $(,)?) => {
		let $crate::hooks::OwnedHookContext::FocusGained = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
	};
	(FocusLost, $owned:ident $(,)?) => {
		let $crate::hooks::OwnedHookContext::FocusLost = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
	};
	(BufferOpen, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::BufferOpen { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(BufferWritePre, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::BufferWritePre { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(BufferWrite, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::BufferWrite { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(BufferClose, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::BufferClose { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(BufferChange, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::BufferChange { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(ModeChange, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::ModeChange { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(CursorMove, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::CursorMove { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(SelectionChange, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::SelectionChange { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
	(WindowResize, $owned:ident, $( $param:ident : $ty:ty ),* $(,)?) => {
		let $crate::hooks::OwnedHookContext::WindowResize { $($param,)* .. } = $owned else {
			return $crate::hooks::HookResult::Continue;
		};
		$(let $param: $ty = $crate::__hook_param_expr!($ty, $param); )*
	};
}

/// Define a hook and register it in the [`HOOKS`](crate::hooks::HOOKS) slice.
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
				$ctx: &mut $crate::hooks::MutableHookContext,
			) -> $crate::hooks::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::hooks::HOOKS)]
			static [<HOOK_ $name>]: $crate::hooks::HookDef = $crate::hooks::HookDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				event: $crate::hooks::HookEvent::$event,
				description: $desc,
				priority: $priority,
				mutability: $crate::hooks::HookMutability::Mutable,
				handler: $crate::hooks::HookHandler::Mutable([<hook_handler_ $name>]),
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
			fn [<hook_handler_ $name>]($ctx: &$crate::hooks::HookContext) -> $crate::hooks::HookAction {
				let result = { $body };
				::core::convert::Into::into(result)
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::hooks::HOOKS)]
			static [<HOOK_ $name>]: $crate::hooks::HookDef = $crate::hooks::HookDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				event: $crate::hooks::HookEvent::$event,
				description: $desc,
				priority: $priority,
				mutability: $crate::hooks::HookMutability::Immutable,
				handler: $crate::hooks::HookHandler::Immutable([<hook_handler_ $name>]),
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
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async || $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
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
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
	($name:ident, $event:ident, $priority:expr, $desc:expr, async |$($param:ident : $ty:ty),*| $body:expr) => {
		$crate::hook!($name, $event, $priority, $desc, |ctx| {
			let owned = ctx.to_owned();
			$crate::hooks::HookAction::Async(::std::boxed::Box::pin(async move {
				$crate::__async_hook_extract!($event, owned, $($param : $ty),*);
				let result = { $body };
				::core::convert::Into::into(result)
			}))
		});
	};
}
