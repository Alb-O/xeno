//! Internal helper macros.
//!
//! These macros are used by the public registration macros and are not
//! intended for direct use.

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

#[doc(hidden)]
#[macro_export]
macro_rules! __hook_param_expr {
	(Option<& $inner:ty>, $value:ident) => {
		$value.as_deref()
	};
	(Option < & $inner:ty >, $value:ident) => {
		$value.as_deref()
	};
	(& $inner:ty, $value:ident) => {
		&$value
	};
	(&$inner:ty, $value:ident) => {
		&$value
	};
	($ty:ty, $value:ident) => {
		$value
	};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __hook_borrowed_ty {
	(Path) => {
		&'a ::std::path::Path
	};
	(RopeSlice) => {
		::ropey::RopeSlice<'a>
	};
	(OptionStr) => {
		::core::option::Option<&'a str>
	};
	($ty:ty) => {
		$ty
	};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __hook_owned_ty {
	(Path) => {
		::std::path::PathBuf
	};
	(RopeSlice) => {
		::std::string::String
	};
	(OptionStr) => {
		::core::option::Option<::std::string::String>
	};
	($ty:ty) => {
		$ty
	};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __hook_owned_value {
	(Path, $value:ident) => {
		$value.to_path_buf()
	};
	(RopeSlice, $value:ident) => {
		$value.to_string()
	};
	(OptionStr, $value:ident) => {
		$value.map(::std::string::String::from)
	};
	($ty:ty, $value:ident) => {
		$value.clone()
	};
}
