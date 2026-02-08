//! Helpers for building registry metadata and interning keys.
//!
//! These helpers implement the standard "Stage C" key construction, ensuring
//! that secondary keys (user keys and domain-specific lookup keys) are merged,
//! sorted, and interned consistently across all domains.

use crate::core::index::build::RegistryMetaRef;
use crate::core::{CapabilitySet, FrozenInterner, RegistryMeta, Symbol, SymbolList};

/// Collects all strings from metadata and extra keys for interning.
pub fn collect_meta_strings<'a>(
	meta: &RegistryMetaRef<'a>,
	sink: &mut Vec<&'a str>,
	extra_keys: impl IntoIterator<Item = &'a str>,
) {
	sink.push(meta.id);
	sink.push(meta.name);
	sink.push(meta.description);
	meta.keys.for_each(|k| sink.push(k));
	for key in extra_keys {
		sink.push(key);
	}
}

/// Builds a symbolized [`RegistryMeta`] and interns keys into the pool.
pub fn build_meta<'a>(
	interner: &FrozenInterner,
	key_pool: &mut Vec<Symbol>,
	meta_ref: RegistryMetaRef<'_>,
	extra_keys: impl IntoIterator<Item = &'a str>,
) -> RegistryMeta {
	let start = key_pool.len() as u32;

	// Collect and dedup secondary keys (user-defined keys + domain-specific keys)
	// Note: id and name are handled in Stages A and B respectively; do not include them here.
	let mut keys = meta_ref.keys.to_vec();
	for key in extra_keys {
		keys.push(key);
	}
	keys.sort_unstable();
	keys.dedup();

	for key in keys {
		key_pool.push(interner.get(key).expect("missing interned key"));
	}
	let len = (key_pool.len() as u32 - start) as u16;
	debug_assert!(key_pool.len() as u32 - start <= u16::MAX as u32);

	RegistryMeta {
		id: interner.get(meta_ref.id).expect("missing interned id"),
		name: interner.get(meta_ref.name).expect("missing interned name"),
		description: interner
			.get(meta_ref.description)
			.expect("missing interned description"),
		keys: SymbolList { start, len },
		priority: meta_ref.priority,
		source: meta_ref.source,
		required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
		flags: meta_ref.flags,
	}
}
