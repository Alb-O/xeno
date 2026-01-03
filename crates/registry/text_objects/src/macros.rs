//! Registration macros for text objects.

/// Helper macro to handle optional slice parameters.
#[doc(hidden)]
#[macro_export]
macro_rules! __text_obj_opt_slice {
	() => {
		&[]
	};
	({$val:expr}) => {
		$val
	};
}

/// Helper macro to handle optional value parameters.
#[doc(hidden)]
#[macro_export]
macro_rules! __text_obj_opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Registers a text object in the [`TEXT_OBJECTS`](crate::TEXT_OBJECTS) slice.
///
/// # Example
///
/// ```ignore
/// text_object!(
///     word,
///     { trigger: 'w', description: "Select word" },
///     {
///         inner: word_inner,
///         around: word_around,
///     }
/// );
/// ```
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
			static [<OBJ_ $name>]: $crate::TextObjectDef = $crate::TextObjectDef::new(
				concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
				stringify!($name),
				$crate::__text_obj_opt_slice!($({$aliases})?),
				$desc,
				$crate::__text_obj_opt!($({$priority})?, 0),
				$crate::__text_obj_opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
				$crate::__text_obj_opt_slice!($({$caps})?),
				$crate::__text_obj_opt!($({$flags})?, $crate::flags::NONE),
				$trigger,
				$crate::__text_obj_opt_slice!($({$alt_triggers})?),
				$inner,
				$around,
			);
		}
	};
}

/// Registers a symmetric text object where inner == around.
///
/// # Example
///
/// ```ignore
/// symmetric_text_object!(
///     line,
///     { trigger: 'l', description: "Select line" },
///     select_line
/// );
/// ```
#[macro_export]
macro_rules! symmetric_text_object {
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
	}, $handler:expr) => {
		$crate::text_object!($name, {
			trigger: $trigger,
			$(alt_triggers: $alt_triggers,)?
			$(aliases: $aliases,)?
			description: $desc
			$(, priority: $priority)?
			$(, caps: $caps)?
			$(, flags: $flags)?
			$(, source: $source)?
		}, {
			inner: $handler,
			around: $handler,
		});
	};
}

/// Registers a bracket-pair text object with surround selection.
///
/// # Example
///
/// ```ignore
/// bracket_pair_object!(parentheses, '(', ')', 'b', &['(', ')']);
/// ```
#[macro_export]
macro_rules! bracket_pair_object {
	($name:ident, $open:expr, $close:expr, $trigger:expr, $alt_triggers:expr) => {
		paste::paste! {
			fn [<$name _inner>](text: ropey::RopeSlice, pos: usize) -> Option<xeno_base::Range> {
				$crate::movement::select_surround_object(
					text,
					xeno_base::Range::point(pos),
					$open,
					$close,
					true,
				)
			}

			fn [<$name _around>](text: ropey::RopeSlice, pos: usize) -> Option<xeno_base::Range> {
				$crate::movement::select_surround_object(
					text,
					xeno_base::Range::point(pos),
					$open,
					$close,
					false,
				)
			}

			$crate::text_object!($name, {
				trigger: $trigger,
				alt_triggers: $alt_triggers,
				description: concat!("Select ", stringify!($name), " block"),
			}, {
				inner: [<$name _inner>],
				around: [<$name _around>],
			});
		}
	};
}
