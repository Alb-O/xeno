pub(crate) mod catalog;

/// Stubs for rustdoc link targets.
#[cfg(doc)]
pub(crate) mod stubs {
	pub fn test_delegator_rule() {}
	pub fn test_mutation_side_effect_invariant() {}
}

#[cfg(doc)]
pub(crate) use stubs::{test_delegator_rule, test_mutation_side_effect_invariant};

#[cfg(test)]
mod proofs;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use proofs::{test_delegator_rule, test_mutation_side_effect_invariant};
