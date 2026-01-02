//! Internal helper macros.
//!
//! These macros are used by the public registration macros and are not
//! intended for direct use.

/// Selects a provided value or falls back to a default.
///
/// Used by registration macros for optional fields like `priority`, `flags`, etc.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt {
	({$val:expr}, $default:expr) => {
		$val
	};
	(, $default:expr) => {
		$default
	};
}

/// Selects a provided slice or returns an empty slice.
///
/// Used by registration macros for optional array fields like `aliases`, `caps`.
#[doc(hidden)]
#[macro_export]
macro_rules! __opt_slice {
	({$val:expr}) => {
		$val
	};
	() => {
		&[]
	};
}
