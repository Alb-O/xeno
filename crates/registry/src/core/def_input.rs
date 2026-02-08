//! Generic registry input type for static and dynamic definitions.

use crate::core::index::{BuildEntry, RegistryMetaRef};
use crate::core::{FrozenInterner, RegistryEntry, Symbol};

/// Represents an inhabitant-free type for domains that don't support linked definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoLinked {}

impl<Out: RegistryEntry> BuildEntry<Out> for NoLinked {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match *self {}
	}

	fn short_desc_str(&self) -> &str {
		match *self {}
	}

	fn collect_strings<'a>(&'a self, _sink: &mut Vec<&'a str>) {
		match *self {}
	}

	fn build(&self, _interner: &FrozenInterner, _key_pool: &mut Vec<Symbol>) -> Out {
		match *self {}
	}
}

/// Unified registry input wrapper for static or linked definitions.
#[derive(Clone)]
pub enum DefInput<S, L = NoLinked> {
	/// Static definition authored via macro.
	Static(S),
	/// Linked definition assembled from dynamic metadata (e.g. KDL).
	Linked(L),
}

impl<Out, S, L> BuildEntry<Out> for DefInput<S, L>
where
	Out: RegistryEntry,
	S: BuildEntry<Out>,
	L: BuildEntry<Out>,
{
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(s) => s.meta_ref(),
			Self::Linked(l) => l.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(s) => s.short_desc_str(),
			Self::Linked(l) => l.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(s) => s.collect_strings(sink),
			Self::Linked(l) => l.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> Out {
		match self {
			Self::Static(s) => s.build(interner, key_pool),
			Self::Linked(l) => l.build(interner, key_pool),
		}
	}
}

impl<S, L> From<S> for DefInput<S, L> {
	fn from(s: S) -> Self {
		Self::Static(s)
	}
}
