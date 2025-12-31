//! Event definition macro.
//!
//! The [`events!`] macro generates `HookEvent`, `HookEventData`, and `OwnedHookContext`
//! types from a declarative event list.

#[macro_export]
macro_rules! events {
	(
		$(
			$(#[$meta:meta])*
			$event:ident => $event_str:literal
			$( { $( $field:ident : $ty:tt ),* $(,)? } )?
		),* $(,)?
	) => {
		#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
		pub enum HookEvent {
			$(
				$(#[$meta])*
				$event,
			)*
		}

		impl HookEvent {
			pub fn as_str(&self) -> &'static str {
				match self {
					$(HookEvent::$event => $event_str,)*
				}
			}
		}

		/// Event-specific data for hooks.
		///
		/// Contains the payload for each hook event type.
		pub enum HookEventData<'a> {
			$(
				$(#[$meta])*
				$event $( { $( $field: $crate::__hook_borrowed_ty!($ty) ),* } )?,
			)*
		}

		impl<'a> HookEventData<'a> {
			/// Returns the event type for this data.
			pub fn event(&self) -> HookEvent {
				match self {
					$(HookEventData::$event $( { $( $field: _ ),* } )? => HookEvent::$event,)*
				}
			}

			/// Creates an owned version of this event data for use in async hooks.
			///
			/// Copies all data so it can be moved into a future.
			pub fn to_owned(&self) -> OwnedHookContext {
				OwnedHookContext::from(self)
			}
		}

		impl<'a> From<&HookEventData<'a>> for OwnedHookContext {
			fn from(data: &HookEventData<'a>) -> Self {
				match data {
					$(
						HookEventData::$event $( { $( $field ),* } )? => {
							OwnedHookContext::$event $( { $( $field: $crate::__hook_owned_value!($ty, $field) ),* } )?
						}
					),*
				}
			}
		}

		/// Owned version of [`HookContext`] for async hook handlers.
		///
		/// Unlike `HookContext` which borrows data, this owns all its data and can be
		/// moved into async futures. Use [`HookContext::to_owned()`] to create one.
		///
		/// # Example
		///
		/// ```ignore
		/// hook!(lsp_open, BufferOpen, 100, "Notify LSP", |ctx| {
		///     let owned = ctx.to_owned();
		///     HookAction::Async(Box::pin(async move {
		///         if let OwnedHookContext::BufferOpen { path, text, file_type } = owned {
		///             lsp.did_open(&path, &text, file_type.as_deref()).await;
		///         }
		///         HookResult::Continue
		///     }))
		/// });
		/// ```
		#[derive(Debug, Clone)]
		pub enum OwnedHookContext {
			$(
				$(#[$meta])*
				$event $( { $( $field: $crate::__hook_owned_ty!($ty) ),* } )?,
			)*
		}

		impl OwnedHookContext {
			/// Returns the event type for this context.
			pub fn event(&self) -> HookEvent {
				match self {
					$(OwnedHookContext::$event $( { $( $field: _ ),* } )? => HookEvent::$event,)*
				}
			}
		}
	};
}
