//! Registration macros for text objects.

/// Registers a handler for a registry-defined text object.
///
/// The metadata (description, trigger, etc.) comes from `text_objects.nuon`; this macro
/// only provides the Rust handlers and creates the inventory linkage.
#[macro_export]
macro_rules! text_object_handler {
	($name:ident, {
		inner: |$ti_text:ident, $ti_pos:ident| $inner_body:expr,
		around: |$ta_text:ident, $ta_pos:ident| $around_body:expr $(,)?
	}) => {
		paste::paste! {
			#[allow(unused_variables)]
			fn [<textobj_inner_ $name>](
				$ti_text: ropey::RopeSlice,
				$ti_pos: usize,
			) -> Option<xeno_primitives::Range> {
				$inner_body
			}

			#[allow(unused_variables)]
			fn [<textobj_around_ $name>](
				$ta_text: ropey::RopeSlice,
				$ta_pos: usize,
			) -> Option<xeno_primitives::Range> {
				$around_body
			}

			#[allow(non_upper_case_globals)]
			pub(crate) static [<TEXTOBJ_HANDLER_ $name>]: $crate::textobj::TextObjectHandlerStatic =
				$crate::textobj::TextObjectHandlerStatic {
					name: stringify!($name),
					crate_name: env!("CARGO_PKG_NAME"),
					handler: $crate::textobj::handler::TextObjectHandlers {
						inner: [<textobj_inner_ $name>],
						around: [<textobj_around_ $name>],
					},
				};

			inventory::submit!($crate::textobj::TextObjectHandlerReg(&[<TEXTOBJ_HANDLER_ $name>]));
		}
	};
}
