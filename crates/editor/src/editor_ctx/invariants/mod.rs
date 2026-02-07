pub(crate) mod catalog;

/// Stubs for rustdoc link targets.
#[cfg(doc)]
pub(crate) mod stubs {
	pub fn test_honesty_rule() {}
	pub fn test_single_path_side_effects() {}
}

#[cfg(doc)]
pub(crate) use stubs::{test_honesty_rule, test_single_path_side_effects};

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{test_honesty_rule, test_single_path_side_effects};
