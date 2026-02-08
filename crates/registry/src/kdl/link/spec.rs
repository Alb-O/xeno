use std::sync::Arc;

use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistrySource};
use crate::kdl::types::MetaCommonRaw;

/// Domain-specific hooks used by generic KDL-to-handler linker.
pub trait DomainLinkSpec {
	type Meta;
	type HandlerFn: Copy + 'static;
	type Entry: crate::core::RegistryEntry;
	type Payload: LinkedPayload<Self::Entry>;

	const WHAT: &'static str;
	const CANONICAL_PREFIX: &'static str;

	fn common(meta: &Self::Meta) -> &MetaCommonRaw;

	/// Domain-level validation of common metadata constraints.
	fn validate_meta(_meta: &Self::Meta) {}

	/// Resolves required capabilities for this domain.
	fn required_caps(meta: &Self::Meta) -> Vec<crate::core::Capability> {
		Self::common(meta)
			.caps
			.iter()
			.map(|c| super::parse::parse_capability(c))
			.collect()
	}

	fn short_desc(meta: &Self::Meta) -> String {
		let common = Self::common(meta);
		common
			.short_desc
			.clone()
			.unwrap_or_else(|| common.description.clone())
	}

	fn build_payload(
		meta: &Self::Meta,
		handler: Self::HandlerFn,
		canonical_id: Arc<str>,
	) -> Self::Payload;
}

/// Generic linker for KDL metadata and inventory-collected handler statics.
pub fn link_domain<D: DomainLinkSpec>(
	metas: &[D::Meta],
	handlers: impl Iterator<Item = &'static crate::core::HandlerStatic<D::HandlerFn>>,
) -> Vec<LinkedDef<D::Payload>> {
	super::common::link_by_name(
		metas,
		handlers,
		|m| D::common(m).name.as_str(),
		|h| h.name,
		|meta, handler| {
			D::validate_meta(meta);
			let common = D::common(meta);
			let id = format!("{}{}", D::CANONICAL_PREFIX, common.name);
			let canonical_id = Arc::from(id.as_str());

			LinkedDef {
				meta: LinkedMetaOwned {
					id,
					name: common.name.clone(),
					keys: common.keys.clone(),
					description: common.description.clone(),
					priority: common.priority,
					source: RegistrySource::Crate(handler.crate_name),
					required_caps: D::required_caps(meta),
					flags: common.flags,
					short_desc: Some(D::short_desc(meta)),
				},
				payload: D::build_payload(meta, handler.handler, canonical_id),
			}
		},
		D::WHAT,
	)
}
