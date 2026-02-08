use crate::core::capability::Capability;
use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{FrozenInterner, RegistryEntry, RegistryMeta, RegistrySource, Symbol};

#[derive(Clone)]
pub struct LinkedMetaOwned {
	pub id: String,
	pub name: String,
	pub keys: Vec<String>,
	pub description: String,
	pub priority: i16,
	pub source: RegistrySource,
	pub required_caps: Vec<Capability>,
	pub flags: u32,
	pub short_desc: Option<String>,
}

pub trait LinkedPayload<Out: RegistryEntry>: Clone + Send + Sync + 'static {
	/// default: use meta.name
	fn short_desc<'a>(&'a self, meta: &'a LinkedMetaOwned) -> &'a str {
		meta.short_desc.as_deref().unwrap_or(meta.name.as_str())
	}

	/// Stage C “extra keys” (e.g. options’ kdl_key). Default none.
	fn collect_extra_keys<'a>(&'a self, _sink: &mut Vec<&'a str>) {}

	/// Any extra strings that must be interned beyond meta/extra_keys/short_desc.
	fn collect_payload_strings<'a>(&'a self, _sink: &mut Vec<&'a str>) {}

	/// Construct the final entry. `short_desc` is interned from `short_desc(meta)`.
	fn build_entry(&self, interner: &FrozenInterner, meta: RegistryMeta, short_desc: Symbol)
	-> Out;
}

#[derive(Clone)]
pub struct LinkedDef<P> {
	pub meta: LinkedMetaOwned,
	pub payload: P,
}

impl<P, Out> BuildEntry<Out> for LinkedDef<P>
where
	Out: RegistryEntry,
	P: LinkedPayload<Out>,
{
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: &self.meta.id,
			name: &self.meta.name,
			keys: StrListRef::Owned(&self.meta.keys),
			description: &self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			required_caps: &self.meta.required_caps,
			flags: self.meta.flags,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.payload.short_desc(&self.meta)
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let mut extra = Vec::new();
		self.payload.collect_extra_keys(&mut extra);

		crate::core::index::meta_build::collect_meta_strings(
			&self.meta_ref(),
			sink,
			extra.iter().copied(),
		);

		sink.push(self.payload.short_desc(&self.meta));
		self.payload.collect_payload_strings(sink);
	}

	fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> Out {
		let mut extra = Vec::new();
		self.payload.collect_extra_keys(&mut extra);

		let meta = crate::core::index::meta_build::build_meta(
			interner,
			key_pool,
			self.meta_ref(),
			extra.iter().copied(),
		);

		let short_desc = interner
			.get(self.payload.short_desc(&self.meta))
			.expect("missing interned short_desc");

		self.payload.build_entry(interner, meta, short_desc)
	}
}
