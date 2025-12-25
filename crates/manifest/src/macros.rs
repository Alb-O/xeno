//! Extension definition macros for ergonomic registration.

/// Define a language and register it in the LANGUAGES slice.
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
                grammar: $crate::language!(@opt_static $({$grammar})?),
                scope: $crate::language!(@opt_static $({$scope})?),
                extensions: $crate::language!(@opt_slice $({$ext})?),
                filenames: $crate::language!(@opt_slice $({$fnames})?),
                globs: $crate::language!(@opt_slice $({$globs})?),
                shebangs: $crate::language!(@opt_slice $({$shebangs})?),
                first_line_patterns: $crate::language!(@opt_slice $({$patterns})?),
                injection_regex: $crate::language!(@opt_static $({$injection})?),
                comment_tokens: $crate::language!(@opt_slice $({$comments})?),
                block_comment: $crate::language!(@opt_tuple $({$block})?),
                description: $desc,
                priority: $crate::language!(@opt $({$priority})?, 0),
                source: $crate::language!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
            };
        }
    };
    (@opt_static {$val:expr}) => { Some($val) };
    (@opt_static) => { None };
    (@opt_slice {$val:expr}) => { $val };
    (@opt_slice) => { &[] };
    (@opt_tuple {$val:expr}) => { Some($val) };
    (@opt_tuple) => { None };
    (@opt {$val:expr}, $default:expr) => { $val };
    (@opt , $default:expr) => { $default };
}

/// Define an option and register it in the OPTIONS slice.
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
			#[linkme::distributed_slice($crate::COMMANDS)]
			static [<CMD_ $name>]: $crate::CommandDef = $crate::CommandDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::command!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: $handler,
				user_data: None,
				priority: $crate::command!(@opt $({$priority})?, 0),
				source: $crate::command!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::command!(@opt $({$caps})?, &[]),
				flags: $crate::command!(@opt $({$flags})?, $crate::flags::NONE),
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
			#[linkme::distributed_slice($crate::ACTIONS)]
			static [<ACTION_ $name>]: $crate::actions::ActionDef = $crate::actions::ActionDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::action!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: $handler,
				priority: $crate::action!(@opt $({$priority})?, 0),
				source: $crate::action!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::action!(@opt $({$caps})?, &[]),
				flags: $crate::action!(@opt $({$flags})?, $crate::flags::NONE),
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
				description: $desc,
				priority: $crate::action!(@opt $({$priority})?, 0),
				caps: $crate::action!(@opt $({$caps})?, &[]),
				flags: $crate::action!(@opt $({$flags})?, $crate::flags::NONE),
				source: $crate::action!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
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
			flags: $crate::action!(@opt $({$flags})?, $crate::flags::NONE),
			source: $crate::action!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
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
			fn [<hook_handler_ $name>]($ctx: &$crate::hooks::HookContext) {
				$body
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
			#[linkme::distributed_slice($crate::TEXT_OBJECTS)]
			static [<OBJ_ $name>]: $crate::TextObjectDef = $crate::TextObjectDef {
				id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				name: stringify!($name),
				aliases: $crate::text_object!(@opt $({$aliases})?, &[]),
				trigger: $trigger,
				alt_triggers: $crate::text_object!(@opt $({$alt_triggers})?, &[]),
				description: $desc,
				inner: $inner,
				around: $around,
				priority: $crate::text_object!(@opt $({$priority})?, 0),
				source: $crate::text_object!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::text_object!(@opt $({$caps})?, &[]),
				flags: $crate::text_object!(@opt $({$flags})?, $crate::flags::NONE),
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
				aliases: $crate::motion!(@opt $({$aliases})?, &[]),
				description: $desc,
				handler: [<motion_handler_ $name>],
				priority: $crate::motion!(@opt $({$priority})?, 0),
				source: $crate::motion!(@opt $({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				required_caps: $crate::motion!(@opt $({$caps})?, &[]),
				flags: $crate::motion!(@opt $({$flags})?, $crate::flags::NONE),
			};
		}
	};
	(@opt {$val:expr}, $default:expr) => { $val };
	(@opt , $default:expr) => { $default };
}

pub use crate::{action, command, hook, language, motion, option, text_object};
