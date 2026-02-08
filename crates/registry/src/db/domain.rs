//! Domain specification trait for registry types.
//!
//! Lives in the `db` module because it wires domain-specific definitions to the
//! central [`crate::db::builder::RegistryDbBuilder`].

use crate::core::DenseId;
use crate::core::index::{BuildEntry, RegistryBuilder};
use crate::core::traits::RegistryEntry;

/// Trait for defining the metadata and behavior of a registry domain.
pub trait DomainSpec {
	/// The input type accepted by the builder (usually an enum wrapping static/linked defs).
	type Input: BuildEntry<Self::Entry> + Send + Sync + 'static;
	/// The runtime entry type stored in the registry index.
	type Entry: RegistryEntry + Send + Sync + 'static;
	/// The dense ID type used for O(1) table lookups.
	type Id: DenseId;

	/// Static definition type (built-ins).
	type StaticDef: 'static;
	/// Linked definition type (from compiled specs).
	type LinkedDef;

	/// Converts a static definition to the builder input type.
	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input;
	/// Converts a linked definition to the builder input type.
	fn linked_to_input(def: Self::LinkedDef) -> Self::Input;

	/// human-readable label for the domain (e.g., "actions").
	const LABEL: &'static str;

	/// Returns a mutable reference to the domain-specific builder.
	fn builder(
		db: &mut crate::db::builder::RegistryDbBuilder,
	) -> &mut RegistryBuilder<Self::Input, Self::Entry, Self::Id>;

	/// Domain-specific side effects to perform when a new definition is pushed.
	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}
