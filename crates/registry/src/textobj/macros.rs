//! Registration macros for text objects.

/// Defines a text object.
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
			pub static [<OBJ_ $name>]: $crate::textobj::TextObjectDef = $crate::textobj::TextObjectDef::new(
				$crate::textobj::RegistryMeta {
					id: concat!(env!("CARGO_PKG_NAME"), "::", stringify!($name)),
					name: stringify!($name),
					aliases: $crate::xeno_registry_core::__reg_opt_slice!($({$aliases})?),
					description: $desc,
					priority: $crate::xeno_registry_core::__reg_opt!($({$priority})?, 0),
					source: $crate::xeno_registry_core::__reg_opt!($({$source})?, $crate::RegistrySource::Crate(env!("CARGO_PKG_NAME"))),
					required_caps: $crate::xeno_registry_core::__reg_opt_slice!($({$caps})?),
					flags: $crate::xeno_registry_core::__reg_opt!($({$flags})?, $crate::motions::flags::NONE),
				},
				$trigger,
				$crate::xeno_registry_core::__reg_opt_slice!($({$alt_triggers})?),
				$inner,
				$around,
			);

			inventory::submit! { $crate::inventory::Reg(&[<OBJ_ $name>]) }
		}
	};
}

/// Registers a symmetric text object where inner == around.
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
		$crate::textobj::text_object!($name, {
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
#[macro_export]
macro_rules! bracket_pair_object {
	($name:ident, $open:expr, $close:expr, $trigger:expr, $alt_triggers:expr) => {
		paste::paste! {
			fn [<$name _inner>](text: ropey::RopeSlice, pos: usize) -> Option<xeno_primitives::Range> {
				$crate::textobj::movement::select_surround_object(
					text,
					xeno_primitives::Range::point(pos),
					$open,
					$close,
					true,
				)
			}

			fn [<$name _around>](text: ropey::RopeSlice, pos: usize) -> Option<xeno_primitives::Range> {
				$crate::textobj::movement::select_surround_object(
					text,
					xeno_primitives::Range::point(pos),
					$open,
					$close,
					false,
				)
			}

			$crate::textobj::text_object!($name, {
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
