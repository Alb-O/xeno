use crate::core::capability::Capability;
use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{RegistryEntry, RegistryMeta, RegistrySource, Symbol};

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
	fn collect_extra_keys<'b>(&'b self, _collector: &mut crate::core::index::StringCollector<'_, 'b>) {}

	/// Any extra strings that must be interned beyond meta/extra_keys/short_desc.
	fn collect_payload_strings<'b>(&'b self, _collector: &mut crate::core::index::StringCollector<'_, 'b>) {}

	/// Construct the final entry. `short_desc` is interned from `short_desc(meta)`.
	fn build_entry(&self, ctx: &mut dyn BuildCtx, meta: RegistryMeta, short_desc: Symbol) -> Out;
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

	fn collect_payload_strings<'b>(&'b self, collector: &mut crate::core::index::StringCollector<'_, 'b>) {
		self.payload.collect_extra_keys(collector);
		self.payload.collect_payload_strings(collector);
	}

	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> Out {
		let mut extra_strings = Vec::new();
		{
			let mut collector = crate::core::index::StringCollector(&mut extra_strings);
			self.payload.collect_extra_keys(&mut collector);
		}

		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), extra_strings.iter().copied());

		let short_desc = ctx.intern(self.payload.short_desc(&self.meta));

		self.payload.build_entry(ctx, meta, short_desc)
	}
}
