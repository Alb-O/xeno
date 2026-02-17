//! Domain specification trait for registry types.
//!
//! This trait is intentionally minimal. Domain modules provide only the pieces
//! needed for `RegistryDbBuilder` wiring and optional push-time hooks.

use crate::core::DenseId;
use crate::core::index::{BuildEntry, RegistryBuilder, RegistryIndex};
use crate::core::traits::RegistryEntry;

/// Trait for defining the metadata and behavior of a registry domain.
pub trait DomainSpec {
	/// The input type accepted by the builder (usually an enum wrapping static/linked defs).
	type Input: BuildEntry<Self::Entry> + Send + Sync + 'static;
	/// The runtime entry type stored in the registry index.
	type Entry: RegistryEntry + Send + Sync + 'static;
	/// The dense ID type used for O(1) table lookups.
	type Id: DenseId;
	/// The runtime container type exposed from [`crate::db::RegistryDb`].
	type Runtime: Send + Sync + 'static;

	/// human-readable label for the domain (e.g., "actions").
	const LABEL: &'static str;

	/// Returns a mutable reference to the domain-specific builder.
	fn builder(db: &mut crate::db::builder::RegistryDbBuilder) -> &mut RegistryBuilder<Self::Input, Self::Entry, Self::Id>;
	/// Builds the runtime container from an immutable index.
	fn into_runtime(index: RegistryIndex<Self::Entry, Self::Id>) -> Self::Runtime;

	/// Domain-specific side effects to perform when a new definition is pushed.
	fn on_push(_db: &mut crate::db::builder::RegistryDbBuilder, _input: &Self::Input) {}
}
