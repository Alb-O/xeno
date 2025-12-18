//! Extension definition macros for ergonomic registration.
//!
//! This module provides declarative macros that simplify registering
//! extensions into the linkme distributed slices.
//!
//! # Examples
//!
//! ```ignore
//! use tome_core::ext::macros::*;
//!
//! // Define a filetype
//! filetype!(rust, {
//!     extensions: &["rs"],
//!     description: "Rust source file",
//! });
//!
//! // Define an option
//! option!(tab_width, Int, 4, Buffer, "Width of a tab character");
//!
//! // Define a keybinding
//! keybind!(Normal, Key::char('h'), "move_left");
//! ```

/// Define a file type and register it in the FILE_TYPES slice.
///
/// # Examples
///
/// ```ignore
/// filetype!(rust, {
///     extensions: &["rs"],
///     filenames: &[],
///     first_line_patterns: &[],
///     description: "Rust source file",
/// });
///
/// // Minimal form (optional fields default to empty)
/// filetype!(go, {
///     extensions: &["go"],
///     description: "Go source file",
/// });
/// ```
#[macro_export]
macro_rules! filetype {
    ($name:ident, {
        extensions: $ext:expr,
        $(filenames: $fnames:expr,)?
        $(first_line_patterns: $patterns:expr,)?
        description: $desc:expr $(,)?
    }) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::FILE_TYPES)]
            static [<FT_ $name:upper>]: $crate::ext::FileTypeDef = $crate::ext::FileTypeDef {
                name: stringify!($name),
                extensions: $ext,
                filenames: $crate::filetype!(@opt_field $($fnames)?),
                first_line_patterns: $crate::filetype!(@opt_field $($patterns)?),
                description: $desc,
            };
        }
    };
    (@opt_field $val:expr) => { $val };
    (@opt_field) => { &[] };
}

/// Define an option and register it in the OPTIONS slice.
///
/// # Examples
///
/// ```ignore
/// option!(tab_width, Int, 4, Buffer, "Width of a tab character");
/// option!(use_tabs, Bool, false, Buffer, "Use tabs for indentation");
/// option!(theme, String, "default".to_string(), Global, "Color theme name");
/// ```
#[macro_export]
macro_rules! option {
    ($name:ident, Bool, $default:expr, $scope:ident, $desc:expr) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::options::OPTIONS)]
            static [<OPT_ $name:upper>]: $crate::ext::options::OptionDef = $crate::ext::options::OptionDef {
                name: stringify!($name),
                description: $desc,
                value_type: $crate::ext::options::OptionType::Bool,
                default: || $crate::ext::options::OptionValue::Bool($default),
                scope: $crate::ext::options::OptionScope::$scope,
            };
        }
    };
    ($name:ident, Int, $default:expr, $scope:ident, $desc:expr) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::options::OPTIONS)]
            static [<OPT_ $name:upper>]: $crate::ext::options::OptionDef = $crate::ext::options::OptionDef {
                name: stringify!($name),
                description: $desc,
                value_type: $crate::ext::options::OptionType::Int,
                default: || $crate::ext::options::OptionValue::Int($default),
                scope: $crate::ext::options::OptionScope::$scope,
            };
        }
    };
    ($name:ident, String, $default:expr, $scope:ident, $desc:expr) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::options::OPTIONS)]
            static [<OPT_ $name:upper>]: $crate::ext::options::OptionDef = $crate::ext::options::OptionDef {
                name: stringify!($name),
                description: $desc,
                value_type: $crate::ext::options::OptionType::String,
                default: || $crate::ext::options::OptionValue::String($default),
                scope: $crate::ext::options::OptionScope::$scope,
            };
        }
    };
}

/// Define a command and register it in the COMMANDS slice.
///
/// # Examples
///
/// ```ignore
/// command!(write, &["w"], "Write buffer to file", |ctx| {
///     ctx.editor.save()?;
///     Ok(CommandOutcome::Ok)
/// });
/// ```
#[macro_export]
macro_rules! command {
	($name:ident, $aliases:expr, $desc:expr, $handler:expr) => {
		paste::paste! {
			#[linkme::distributed_slice($crate::ext::COMMANDS)]
			static [<CMD_ $name:upper>]: $crate::ext::CommandDef = $crate::ext::CommandDef {
				name: stringify!($name),
				aliases: $aliases,
				description: $desc,
				handler: $handler,
			};
		}
	};
}

/// Define an action and register it in the ACTIONS slice.
///
/// # Examples
///
/// ```ignore
/// action!(move_left, "Move left", |ctx| {
///     cursor_move_action(ctx, "move_left")
/// });
///
/// // For simple result actions:
/// action!(quit, "Quit editor", ActionResult::Quit);
/// ```
#[macro_export]
macro_rules! action {
    ($name:ident, $desc:expr, |$ctx:ident| $body:expr) => {
        paste::paste! {
            fn [<handler_ $name>]($ctx: &$crate::ext::actions::ActionContext) -> $crate::ext::actions::ActionResult {
                $body
            }

            #[linkme::distributed_slice($crate::ext::actions::ACTIONS)]
            static [<ACTION_ $name:upper>]: $crate::ext::actions::ActionDef = $crate::ext::actions::ActionDef {
                name: stringify!($name),
                description: $desc,
                handler: [<handler_ $name>],
            };
        }
    };
    ($name:ident, $desc:expr, $result:expr) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::actions::ACTIONS)]
            static [<ACTION_ $name:upper>]: $crate::ext::actions::ActionDef = $crate::ext::actions::ActionDef {
                name: stringify!($name),
                description: $desc,
                handler: |_ctx| $result,
            };
        }
    };
}

/// Define a hook and register it in the HOOKS slice.
///
/// # Examples
///
/// ```ignore
/// hook!(log_mode_change, ModeChange, 100, "Log mode changes", |ctx| {
///     if let HookContext::ModeChange { old_mode, new_mode } = ctx {
///         println!("Mode: {:?} -> {:?}", old_mode, new_mode);
///     }
/// });
/// ```
#[macro_export]
macro_rules! hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$ctx:ident| $body:expr) => {
		paste::paste! {
			fn [<hook_handler_ $name>]($ctx: &$crate::ext::hooks::HookContext) {
				$body
			}

			#[linkme::distributed_slice($crate::ext::hooks::HOOKS)]
			static [<HOOK_ $name:upper>]: $crate::ext::hooks::HookDef = $crate::ext::hooks::HookDef {
				name: stringify!($name),
				event: $crate::ext::hooks::HookEvent::$event,
				description: $desc,
				priority: $priority,
				handler: [<hook_handler_ $name>],
			};
		}
	};
}

/// Define a keybinding and register it in the KEYBINDINGS slice.
///
/// # Examples
///
/// ```ignore
/// keybind!(Normal, Key::char('h'), "move_left");
/// keybind!(Normal, Key::char('w'), "next_word_start", priority: 50);
/// ```
#[macro_export]
macro_rules! keybind {
    ($mode:ident, $key:expr, $action:expr) => {
        $crate::keybind!($mode, $key, $action, priority: 100);
    };
    ($mode:ident, $key:expr, $action:expr, priority: $priority:expr) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::ext::keybindings::[<KEYBINDINGS_ $mode:upper>])]
            static [<KB_ $mode:upper _ $action:upper>]: $crate::ext::keybindings::KeyBindingDef =
                $crate::ext::keybindings::KeyBindingDef {
                    mode: $crate::ext::keybindings::BindingMode::$mode,
                    key: $key,
                    action: $action,
                    priority: $priority,
                };
        }
    };
}

/// Define a text object and register it in the TEXT_OBJECTS slice.
///
/// # Examples
///
/// ```ignore
/// text_object!(word, 'w', &[], "Select word", {
///     inner: |text, pos| Some(select_word_inner(text, pos)),
///     around: |text, pos| Some(select_word_around(text, pos)),
/// });
/// ```
#[macro_export]
macro_rules! text_object {
	($name:ident, $trigger:expr, $alt_triggers:expr, $desc:expr, {
        inner: $inner:expr,
        around: $around:expr $(,)?
    }) => {
		paste::paste! {
			#[linkme::distributed_slice($crate::ext::TEXT_OBJECTS)]
			static [<OBJ_ $name:upper>]: $crate::ext::TextObjectDef = $crate::ext::TextObjectDef {
				name: stringify!($name),
				trigger: $trigger,
				alt_triggers: $alt_triggers,
				description: $desc,
				inner: $inner,
				around: $around,
			};
		}
	};
}

/// Define a motion and register it in the MOTIONS slice.
///
/// # Examples
///
/// ```ignore
/// motion!(move_left, "Move left", |text, range, count, extend| {
///     move_horizontally(text, range, Direction::Backward, count, extend)
/// });
/// ```
#[macro_export]
macro_rules! motion {
	($name:ident, $desc:expr, |$text:ident, $range:ident, $count:ident, $extend:ident| $body:expr) => {
		paste::paste! {
			fn [<motion_handler_ $name>](
				$text: ropey::RopeSlice,
				$range: $crate::range::Range,
				$count: usize,
				$extend: bool,
			) -> $crate::range::Range {
				$body
			}

			#[linkme::distributed_slice($crate::ext::MOTIONS)]
			static [<MOTION_ $name:upper>]: $crate::ext::MotionDef = $crate::ext::MotionDef {
				name: stringify!($name),
				description: $desc,
				handler: [<motion_handler_ $name>],
			};
		}
	};
}

pub use crate::{action, command, filetype, hook, keybind, motion, option, text_object};
