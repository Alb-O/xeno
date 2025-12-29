//! Macros for registering editor primitives at compile time.
//!
//! These macros generate static entries in [`linkme`] distributed slices,
//! enabling zero-cost registration of actions, keybindings, motions, hooks,
//! and other extensible editor components.
//!
//! # Primary Macros
//!
//! - [`bound_action!`] - Action with colocated keybindings (preferred)
//! - [`action!`] - Action without keybindings
//! - [`bind!`] - Additional keybindings for existing actions
//! - [`motion!`] - Cursor/selection movement primitives
//! - [`hook!`] - Event lifecycle observers
//! - [`command!`] - Ex-mode commands (`:write`, `:quit`)
//!
//! # Secondary Macros
//!
//! - [`language!`] - Language definitions for syntax highlighting
//! - [`option!`] - Configuration options
//! - [`text_object!`] - Text object selection (`iw`, `a"`, etc.)
//! - [`statusline_segment!`] - Statusline segment definitions
//! - [`full_action!`] - Action + keybinding + result handler combined
//! - [`stub_action!`] - Placeholder for unimplemented features

#[doc(hidden)]
#[macro_export]
macro_rules! __opt {
	({$val:expr}, $default:expr) => { $val };
	(, $default:expr) => { $default };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_slice {
	({$val:expr}) => { $val };
	() => { &[] };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_static {
	({$val:expr}) => { Some($val) };
	() => { None };
}

/// Registers a language definition in the [`LANGUAGES`](crate::LANGUAGES) slice.
///
/// # Example
///
/// ```ignore
/// language!(rust, {
///     grammar: "rust",
///     extensions: &["rs"],
///     comment_tokens: &["//"],
///     block_comment: ("/*", "*/"),
///     description: "Rust source file",
/// });
/// ```
#[macro_export]
macro_rules! language {
	($name:ident, {
		$(grammar: $grammar:expr,)?
		$(scope: $scope:expr,)?
		$(extensions: $ext:expr,)?
		$(filenames: $fnames:expr,)?
		$(globs: $globs:expr,)?
		$(shebangs: $shebangs:expr,)?
		$(first_line_patterns: $patterns:expr,)?
		$(injection_regex: $injection:expr,)?
		$(comment_tokens: $comments:expr,)?
		$(block_comment: $block:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, source: $source:expr)?
		$(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::LANGUAGES)]
			static [<LANG_ $name>]: $crate::LanguageDef = $crate::LanguageDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				grammar: $crate::__opt_static!($({$grammar})?),
				scope: $crate::__opt_static!($({$scope})?),
				extensions: $crate::__opt_slice!($({$ext})?),
				filenames: $crate::__opt_slice!($({$fnames})?),
				globs: $crate::__opt_slice!($({$globs})?),
				shebangs: $crate::__opt_slice!($({$shebangs})?),
				first_line_patterns: $crate::__opt_slice!($({$patterns})?),
				injection_regex: $crate::__opt_static!($({$injection})?),
				comment_tokens: $crate::__opt_slice!($({$comments})?),
				block_comment: $crate::__opt_static!($({$block})?),
				description: $desc,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
			};
		}
	};
}

/// Registers a configuration option in the [`OPTIONS`](crate::options::OPTIONS) slice.
#[macro_export]
macro_rules! option {
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::options::OPTIONS)]
			static [<OPT_ $name>]: $crate::options::OptionDef = $crate::options::OptionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				value_type: $crate::options::OptionType::$type,
				default: || $crate::options::OptionValue::$type($default),
				scope: $crate::options::OptionScope::$scope,
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Registers an ex-mode command in the [`COMMANDS`](crate::COMMANDS) slice.
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
			#[linkme::distributed_slice($crate::COMMANDS)]
			static [<CMD_ $name>]: $crate::CommandDef = $crate::CommandDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				description: $desc,
				handler: $handler,
				user_data: None,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};
}

/// Registers an action in the [`ACTIONS`](crate::ACTIONS) slice.
///
/// Prefer [`bound_action!`] when the action has associated keybindings.
///
/// # Forms
///
/// ```ignore
/// action!(name, { description: "..." }, handler: my_handler);
/// action!(name, { description: "..." }, |ctx| { ... });
/// action!(name, { description: "..." }, result: ActionResult::Variant);
/// ```
#[macro_export]
macro_rules! action {
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
			#[linkme::distributed_slice($crate::ACTIONS)]
			static [<ACTION_ $name>]: $crate::actions::ActionDef = $crate::actions::ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				description: $desc,
				handler: $handler,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, |$ctx:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
				$body
			}

			$crate::action!($name, {
				$(aliases: $aliases,)?
				description: $desc
				$(, priority: $priority)?
				$(, caps: $caps)?
				$(, flags: $flags)?
				$(, source: $source)?
			}, handler: [<handler_ $name>]);
		}
	};

	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, result: $result:expr) => {
		$crate::action!($name, {
			$(aliases: $aliases,)?
			description: $desc
			$(, priority: $priority)?
			$(, caps: $caps)?
			$(, flags: $flags)?
			$(, source: $source)?
		}, |_ctx| $result);
	};
}

/// Define an action with colocated keybindings.
///
/// Bindings use KDL syntax where each line is `mode "key1" "key2" ...`.
///
/// # Example
///
/// ```ignore
/// bound_action!(
///     document_start,
///     description: "Move to document start",
///     bindings: r#"normal "ctrl-home"
/// goto "g" "k""#,
///     |_ctx| ActionResult::Motion(...)
/// );
/// ```
#[macro_export]
macro_rules! bound_action {
	($name:ident,
		description: $desc:expr,
		bindings: $kdl:literal
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?,
		|$ctx:ident| $body:expr
	) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<handler_ $name>]($ctx: &$crate::actions::ActionContext) -> $crate::actions::ActionResult {
				$body
			}

			$crate::bound_action!($name,
				description: $desc,
				bindings: $kdl
				$(, priority: $priority)?
				$(, caps: $caps)?
				$(, flags: $flags)?,
				handler: [<handler_ $name>]
			);
		}
	};

	($name:ident,
		description: $desc:expr,
		bindings: $kdl:literal
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(,)?,
		handler: $handler:expr
	) => {
		$crate::action!($name, {
			description: $desc
			$(, priority: $priority)?
			$(, caps: $caps)?
			$(, flags: $flags)?
		}, handler: $handler);

		evildoer_macro::parse_keybindings!($name, $kdl);
	};
}

/// Register additional keybindings for an existing action.
///
/// # Example
///
/// ```ignore
/// bind!(scroll_down, r#"view "j""#);
/// ```
#[macro_export]
macro_rules! bind {
	($action:ident, $kdl:literal) => {
		evildoer_macro::parse_keybindings!($action, $kdl);
	};
}

/// Define a hook and register it in the [`HOOKS`](crate::hooks::HOOKS) slice.
///
/// # Example
///
/// ```ignore
/// hook!(log_open, BufferOpen, 100, "Log buffer opens", |ctx| {
///     if let HookContext::BufferOpen { path, .. } = ctx {
///         log::info!("Opened: {}", path.display());
///     }
/// });
/// ```
#[macro_export]
macro_rules! hook {
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
				handler: [<hook_handler_ $name>],
				source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Registers a text object in the [`TEXT_OBJECTS`](crate::TEXT_OBJECTS) slice.
#[macro_export]
macro_rules! text_object {
	($name:ident, {
		trigger: $trigger:expr,
		$(alt_triggers: $alt_triggers:expr,)?
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, {
		inner: $inner:expr,
		around: $around:expr $(,)?
	}) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::TEXT_OBJECTS)]
			static [<OBJ_ $name>]: $crate::TextObjectDef = $crate::TextObjectDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				trigger: $trigger,
				alt_triggers: $crate::__opt_slice!($({$alt_triggers})?),
				description: $desc,
				inner: $inner,
				around: $around,
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};
}

/// Registers a motion primitive in the [`MOTIONS`](crate::MOTIONS) slice.
#[macro_export]
macro_rules! motion {
	($name:ident, {
		$(aliases: $aliases:expr,)?
		description: $desc:expr
		$(, priority: $priority:expr)?
		$(, caps: $caps:expr)?
		$(, flags: $flags:expr)?
		$(, source: $source:expr)?
		$(,)?
	}, |$text:ident, $range:ident, $count:ident, $extend:ident| $body:expr) => {
		paste::paste! {
			#[allow(unused_variables, non_snake_case)]
			fn [<motion_handler_ $name>](
				$text: ropey::RopeSlice,
				$range: $crate::Range,
				$count: usize,
				$extend: bool,
			) -> $crate::Range {
				$body
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::MOTIONS)]
			static [<MOTION_ $name>]: $crate::MotionDef = $crate::MotionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::__opt_slice!($({$aliases})?),
				description: $desc,
				handler: [<motion_handler_ $name>],
				priority: $crate::__opt!($({$priority})?, 0),
				source: $crate::__opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::__opt_slice!($({$caps})?),
				flags: $crate::__opt!($({$flags})?, $crate::flags::NONE),
			};
		}
	};
}

/// Registers a statusline segment in the [`STATUSLINE_SEGMENTS`](crate::STATUSLINE_SEGMENTS) slice.
#[macro_export]
macro_rules! statusline_segment {
	($static_name:ident, $name:expr, $position:expr, $priority:expr, $enabled:expr, $render:expr) => {
		#[::linkme::distributed_slice($crate::STATUSLINE_SEGMENTS)]
		static $static_name: $crate::StatuslineSegmentDef = $crate::StatuslineSegmentDef {
			id: $name,
			name: $name,
			position: $position,
			priority: $priority,
			default_enabled: $enabled,
			render: $render,
			source: $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
		};
	};
}

/// Registers a handler for an [`ActionResult`](crate::ActionResult) variant.
#[macro_export]
macro_rules! result_handler {
	($slice:ident, $static_name:ident, $name:literal, $body:expr) => {
		#[::linkme::distributed_slice($crate::actions::$slice)]
		static $static_name: $crate::editor_ctx::ResultHandler = $crate::editor_ctx::ResultHandler {
			name: $name,
			handle: $body,
		};
	};
}

/// Register a complete action with keybinding and result handler.
///
/// Combines action definition, keybinding registration, and result handler
/// in one macro. Used for window/buffer management actions.
#[macro_export]
macro_rules! full_action {
	(
		$name:ident,
		description: $desc:expr,
		key: $key:expr,
		mode: $mode:ident,
		result: $result:ident,
		handler_slice: $slice:ident,
		|$ops:ident| $body:expr
	) => {
		paste::paste! {
			$crate::action!($name, { description: $desc }, result: $crate::actions::ActionResult::$result);

			#[::linkme::distributed_slice($crate::keybindings::[<KEYBINDINGS_ $mode:upper>])]
			static [<KB_ $name:upper>]: $crate::keybindings::KeyBindingDef =
				$crate::keybindings::KeyBindingDef {
					mode: $crate::keybindings::BindingMode::$mode,
					key: $key,
					action: stringify!($name),
					priority: 100,
				};

			#[::linkme::distributed_slice($crate::actions::$slice)]
			static [<HANDLE_ $name:upper>]: $crate::editor_ctx::ResultHandler =
				$crate::editor_ctx::ResultHandler {
					name: stringify!($name),
					handle: |r, ctx, _| {
						use $crate::editor_ctx::MessageAccess;
						if matches!(r, $crate::actions::ActionResult::$result) {
							if let Some($ops) = ctx.buffer_ops() {
								$body;
							} else {
								ctx.notify("warning", "Buffer operations not available");
							}
						}
						$crate::editor_ctx::HandleOutcome::Handled
					},
				};
		}
	};
}

/// Register a stub action that shows "not implemented" when invoked.
#[macro_export]
macro_rules! stub_action {
	(
		$name:ident,
		description: $desc:expr,
		bindings: $kdl:literal,
		result: $result:ident,
		handler_slice: $slice:ident
	) => {
		$crate::bound_action!(
			$name,
			description: $desc,
			bindings: $kdl,
			|_ctx| $crate::actions::ActionResult::$result
		);

		paste::paste! {
			#[::linkme::distributed_slice($crate::actions::$slice)]
			static [<HANDLE_ $name:upper>]: $crate::editor_ctx::ResultHandler =
				$crate::editor_ctx::ResultHandler {
					name: stringify!($name),
					handle: |_, ctx, _| {
						use $crate::editor_ctx::MessageAccess;
						ctx.notify("warning", concat!(stringify!($name), " not yet implemented"));
						$crate::editor_ctx::HandleOutcome::Handled
					},
				};
		}
	};
}

pub use crate::{
	__opt, __opt_slice, __opt_static, action, bind, bound_action, command, full_action, hook,
	language, motion, option, result_handler, statusline_segment, stub_action, text_object,
};
