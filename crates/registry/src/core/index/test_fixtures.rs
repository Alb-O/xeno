//! Shared test helpers for registry index tests.

use crate::core::index::build::{
	BuildCtx, BuildEntry, RegistryMetaRef, StrListRef, StringCollector,
};
use crate::core::{RegistryEntry, RegistryMeta, RegistryMetaStatic, RegistrySource, Symbol};

/// Minimal test entry type for proofs.
pub(crate) struct TestEntry {
	pub meta: RegistryMeta,
}

impl RegistryEntry for TestEntry {
	fn meta(&self) -> &RegistryMeta {
		&self.meta
	}
}

/// Minimal test def type for proofs.
#[derive(Clone)]
pub(crate) struct TestDef {
	pub meta: RegistryMetaStatic,
}

impl BuildEntry<TestEntry> for TestDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: self.meta.required_caps,
			flags: self.meta.flags,
		}
	}
	fn short_desc_str(&self) -> &str {
		self.meta.name
	}
	fn collect_payload_strings<'b>(&'b self, _collector: &mut StringCollector<'_, 'b>) {}
	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> TestEntry {
		TestEntry {
			meta: crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []),
		}
	}
}

pub(crate) fn make_def(id: &'static str, priority: i16) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			keys: &[],
			description: "",
			priority,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	}
}

pub(crate) fn make_def_with_source(
	id: &'static str,
	priority: i16,
	source: RegistrySource,
) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			keys: &[],
			description: "",
			priority,
			source,
			required_caps: &[],
			flags: 0,
		},
	}
}

pub(crate) fn make_def_with_keyes(
	id: &'static str,
	priority: i16,
	keys: &'static [&'static str],
) -> TestDef {
	TestDef {
		meta: RegistryMetaStatic {
			id,
			name: id,
			keys,
			description: "",
			priority,
			source: RegistrySource::Builtin,
			required_caps: &[],
			flags: 0,
		},
	}
}
