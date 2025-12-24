//! Extension definition macros for ergonomic registration.

/// Define a file type and register it in the FILE_TYPES slice.
#[macro_export]
macro_rules! filetype {
    ($name:ident, {
        extensions: $ext:expr,
        $(filenames: $fnames:expr,)?
        $(first_line_patterns: $patterns:expr,)?
        description: $desc:expr
        $(, priority: $priority:expr)?
        $(, source: $source:expr)?
        $(,)?
    }) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            #[linkme::distributed_slice($crate::registry::FILE_TYPES)]
            static [<FT_ $name>]: $crate::registry::FileTypeDef = $crate::registry::FileTypeDef {
                id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
                name: stringify!($name),
                extensions: $ext,
                filenames: $crate::filetype!(@opt_field $($fnames)?),
                first_line_patterns: $crate::filetype!(@opt_field $($patterns)?),
                description: $desc,
                priority: $crate::filetype!(@opt $({$priority})?, 0),
                source: $crate::filetype!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
            };
        }
    };
    (@opt_field $val:expr) => { $val };
    (@opt_field) => { &[] };
    (@opt {$val:expr}, $default:expr) => { $val };
    (@opt , $default:expr) => { $default };
}

/// Define an option and register it in the OPTIONS slice.
#[macro_export]
macro_rules! option {
	($name:ident, $type:ident, $default:expr, $scope:ident, $desc:expr) => {
		paste::paste! {
			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::registry::options::OPTIONS)]
			static [<OPT_ $name>]: $crate::registry::options::OptionDef = $crate::registry::options::OptionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				description: $desc,
				value_type: $crate::registry::options::OptionType::$type,
				default: || $crate::registry::options::OptionValue::$type($default),
				scope: $crate::registry::options::OptionScope::$scope,
				source: $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Define a command and register it in the COMMANDS slice.
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
			#[linkme::distributed_slice($crate::registry::COMMANDS)]
			static [<CMD_ $name>]: $crate::registry::CommandDef = $crate::registry::CommandDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::command!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: $handler,
				user_data: None,
				priority: $crate::command!(@opt $({$priority})?, 0),
				source: $crate::command!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::command!(@opt $({$caps})?, &[]),
				flags: $crate::command!(@opt $({$flags})?, $crate::registry::flags::NONE),
			};
		}
	};
	(@opt {$val:expr}, $default:expr) => { $val };
	(@opt , $default:expr) => { $default };
}

/// Define an action and register it in the ACTIONS slice.
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
			#[linkme::distributed_slice($crate::registry::actions::ACTIONS)]
			static [<ACTION_ $name>]: $crate::registry::actions::ActionDef = $crate::registry::actions::ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::action!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: $handler,
				priority: $crate::action!(@opt $({$priority})?, 0),
				source: $crate::action!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::action!(@opt $({$caps})?, &[]),
				flags: $crate::action!(@opt $({$flags})?, $crate::registry::flags::NONE),
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
			fn [<handler_ $name>]($ctx: &$crate::registry::actions::ActionContext) -> $crate::registry::actions::ActionResult {
				$body
			}
			$crate::action!($name, {
				$(aliases: $aliases,)?
				description: $desc,
				priority: $crate::action!(@opt $({$priority})?, 0),
				caps: $crate::action!(@opt $({$caps})?, &[]),
				flags: $crate::action!(@opt $({$flags})?, $crate::registry::flags::NONE),
				source: $crate::action!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
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
			description: $desc,
			priority: $crate::action!(@opt $({$priority})?, 0),
			caps: $crate::action!(@opt $({$caps})?, &[]),
			flags: $crate::action!(@opt $({$flags})?, $crate::registry::flags::NONE),
			source: $crate::action!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
		}, handler: |_ctx| $result);
	};
	(@opt {$val:expr}, $default:expr) => { $val };
	(@opt , $default:expr) => { $default };
}

/// Define a hook and register it in the HOOKS slice.
#[macro_export]
macro_rules! hook {
	($name:ident, $event:ident, $priority:expr, $desc:expr, |$ctx:ident| $body:expr) => {
		paste::paste! {
			fn [<hook_handler_ $name>]($ctx: &$crate::registry::hooks::HookContext) {
				$body
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::registry::hooks::HOOKS)]
			static [<HOOK_ $name>]: $crate::registry::hooks::HookDef = $crate::registry::hooks::HookDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				event: $crate::registry::hooks::HookEvent::$event,
				description: $desc,
				priority: $priority,
				handler: [<hook_handler_ $name>],
				source: $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME")),
			};
		}
	};
}

/// Define a text object and register it in the TEXT_OBJECTS slice.
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
			#[linkme::distributed_slice($crate::registry::TEXT_OBJECTS)]
			static [<OBJ_ $name>]: $crate::registry::TextObjectDef = $crate::registry::TextObjectDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::text_object!(@opt $({$aliases})?, &[]),
				trigger: $trigger,
				alt_triggers: $crate::text_object!(@opt $({$alt_triggers})?, &[]),
				description: $desc,
				inner: $inner,
				around: $around,
				priority: $crate::text_object!(@opt $({$priority})?, 0),
				source: $crate::text_object!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::text_object!(@opt $({$caps})?, &[]),
				flags: $crate::text_object!(@opt $({$flags})?, $crate::registry::flags::NONE),
			};
		}
	};
	(@opt {$val:expr}, $default:expr) => { $val };
	(@opt , $default:expr) => { $default };
}

/// Define a motion and register it in the MOTIONS slice.
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
				$range: $crate::range::Range,
				$count: usize,
				$extend: bool,
			) -> $crate::range::Range {
				$body
			}

			#[allow(non_upper_case_globals)]
			#[linkme::distributed_slice($crate::registry::MOTIONS)]
			static [<MOTION_ $name>]: $crate::registry::MotionDef = $crate::registry::MotionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::motion!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: [<motion_handler_ $name>],
				priority: $crate::motion!(@opt $({$priority})?, 0),
				source: $crate::motion!(@opt $({$source})?, $crate::registry::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::motion!(@opt $({$caps})?, &[]),
				flags: $crate::motion!(@opt $({$flags})?, $crate::registry::flags::NONE),
			};
		}
	};
	(@opt {$val:expr}, $default:expr) => { $val };
	(@opt , $default:expr) => { $default };
}

pub use crate::{action, command, filetype, hook, motion, option, text_object};
