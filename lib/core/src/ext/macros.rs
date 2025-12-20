//! Extension definition macros for ergonomic registration.

/// Define a file type and register it in the FILE_TYPES slice.
#[macro_export]
macro_rules! filetype {
    ($name:ident, {
        extensions: $ext:expr,
        $(filenames: $fnames:expr,)?
        $(first_line_patterns: $patterns:expr,)?
        description: $desc:expr $(,)?
    }) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::ext::FILE_TYPES)]
            static [<FT_ $name>]: $crate::ext::FileTypeDef = $crate::ext::FileTypeDef {
                id: stringify!($name),
                name: stringify!($name),
                extensions: $ext,
                filenames: $crate::filetype!(@opt_field $($fnames)?),
                first_line_patterns: $crate::filetype!(@opt_field $($patterns)?),
                description: $desc,
                source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
            };
        }
    };
    (@opt_field $val:expr) => { $val };
    (@opt_field) => { &[] };
}

/// Define an option and register it in the OPTIONS slice.
#[macro_export]
macro_rules! option {
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ext::options::OPTIONS)]
			static [<OPT_ $name>]: $crate::ext::options::OptionDef = $crate::ext::options::OptionDef {
				id: stringify!($name),
				name: stringify!($name),
				description: $desc,
				value_type: $crate::ext::options::OptionType::$type,
				default: || $crate::ext::options::OptionValue::$type($default),
				scope: $crate::ext::options::OptionScope::$scope,
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Define a command and register it in the COMMANDS slice.
#[macro_export]
macro_rules! command {
	($name:ident, $aliases:expr, $desc:expr, handler: $handler:expr) => {
		$crate::command!($name, $aliases, $desc, handler: $handler, priority: 0, caps: &[]);
	};
	($name:ident, $aliases:expr, $desc:expr, handler: $handler:expr, priority: $priority:expr, caps: $caps:expr) => {
		paste::paste! {
            #[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ext::COMMANDS)]
			static [<CMD_ $name>]: $crate::ext::CommandDef = $crate::ext::CommandDef {
				id: stringify!($name),
				name: stringify!($name),
				aliases: $aliases,
				description: $desc,
				handler: $handler,
				user_data: None,
				priority: $priority,
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
				required_caps: $caps,
				flags: $crate::ext::flags::NONE,
			};
		}
	};
}

/// Define an action and register it in the ACTIONS slice.
#[macro_export]
macro_rules! action {
    ($name:ident, $desc:expr, |$ctx:ident| $body:expr) => {
        $crate::action!($name, $desc, |$ctx| $body, priority: 0, caps: &[]);
    };
    ($name:ident, $desc:expr, |$ctx:ident| $body:expr, priority: $priority:expr, caps: $caps:expr) => {
        paste::paste! {
            fn [<handler_ $name>]($ctx: &$crate::ext::actions::ActionContext) -> $crate::ext::actions::ActionResult {
                $body
            }

            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::ext::actions::ACTIONS)]
            static [<ACTION_ $name>]: $crate::ext::actions::ActionDef = $crate::ext::actions::ActionDef {
                id: stringify!($name),
                name: stringify!($name),
                description: $desc,
                handler: [<handler_ $name>],
                priority: $priority,
                source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
                required_caps: $caps,
                flags: $crate::ext::flags::NONE,
            };
        }
    };
    ($name:ident, $desc:expr, $result:expr) => {
        $crate::action!($name, $desc, $result, priority: 0, caps: &[]);
    };
    ($name:ident, $desc:expr, $result:expr, priority: $priority:expr, caps: $caps:expr) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::ext::actions::ACTIONS)]
            static [<ACTION_ $name>]: $crate::ext::actions::ActionDef = $crate::ext::actions::ActionDef {
                id: stringify!($name),
                name: stringify!($name),
                description: $desc,
                handler: |_ctx| $result,
                priority: $priority,
                source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
                required_caps: $caps,
                flags: $crate::ext::flags::NONE,
            };
        }
    };
    ($name:ident, $desc:expr, handler: $handler:expr) => {
        $crate::action!($name, $desc, handler: $handler, priority: 0, caps: &[]);
    };
    ($name:ident, $desc:expr, handler: $handler:expr, priority: $priority:expr, caps: $caps:expr) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::ext::actions::ACTIONS)]
            static [<ACTION_ $name>]: $crate::ext::actions::ActionDef = $crate::ext::actions::ActionDef {
                id: stringify!($name),
                name: stringify!($name),
                description: $desc,
                handler: $handler,
                priority: $priority,
                source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
                required_caps: $caps,
                flags: $crate::ext::flags::NONE,
            };
        }
    };
}

/// Define a hook and register it in the HOOKS slice.
#[macro_export]
macro_rules! hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$ctx:ident| $body:expr) => {
		paste::paste! {
			fn [<hook_handler_ $name>]($ctx: &$crate::ext::hooks::HookContext) {
				$body
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ext::hooks::HOOKS)]
			static [<HOOK_ $name>]: $crate::ext::hooks::HookDef = $crate::ext::hooks::HookDef {
				id: stringify!($name),
				name: stringify!($name),
				event: $crate::ext::hooks::HookEvent::$event,
				description: $desc,
				priority: $priority,
				handler: [<hook_handler_ $name>],
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Define a text object and register it in the TEXT_OBJECTS slice.
#[macro_export]
macro_rules! text_object {
	($name:ident, $trigger:expr, $alt_triggers:expr, $desc:expr, {
        inner: $inner:expr,
        around: $around:expr $(,)?
    }) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ext::TEXT_OBJECTS)]
			static [<OBJ_ $name>]: $crate::ext::TextObjectDef = $crate::ext::TextObjectDef {
				id: stringify!($name),
				name: stringify!($name),
				trigger: $trigger,
				alt_triggers: $alt_triggers,
				description: $desc,
				inner: $inner,
				around: $around,
				priority: 0,
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
				flags: $crate::ext::flags::NONE,
			};
		}
	};
}

/// Define a motion and register it in the MOTIONS slice.
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

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::ext::MOTIONS)]
			static [<MOTION_ $name>]: $crate::ext::MotionDef = $crate::ext::MotionDef {
				id: stringify!($name),
				name: stringify!($name),
				description: $desc,
				handler: [<motion_handler_ $name>],
				priority: 0,
				source: $crate::ext::ExtensionSource::Crate(env!("CARGO_PKG_NAME")),
				flags: $crate::ext::flags::NONE,
			};
		}
	};
}

pub use crate::{action, command, filetype, hook, motion, option, text_object};
