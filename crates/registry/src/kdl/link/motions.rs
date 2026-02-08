use super::*;
use crate::core::{LinkedDef, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::{MotionMetaRaw, MotionsBlob};
use crate::motions::handler::MotionHandlerStatic;
use crate::motions::{MotionEntry, MotionHandler};

/// A motion definition assembled from KDL metadata + Rust handler.
pub type LinkedMotionDef = LinkedDef<MotionPayload>;

#[derive(Clone)]
pub struct MotionPayload {
	pub handler: MotionHandler,
}

impl LinkedPayload<MotionEntry> for MotionPayload {
	fn build_entry(
		&self,
		_interner: &FrozenInterner,
		meta: RegistryMeta,
		_short_desc: Symbol,
	) -> MotionEntry {
		MotionEntry {
			meta,
			handler: self.handler,
		}
	}
}

/// Links KDL motion metadata with handler statics, producing `LinkedMotionDef`s.
///
/// Panics if any KDL motion has no matching handler, or vice versa.
pub fn link_motions(
	metadata: &MotionsBlob,
	handlers: impl Iterator<Item = &'static MotionHandlerStatic>,
) -> Vec<LinkedMotionDef> {
	super::spec::link_domain::<MotionLinkSpec>(&metadata.motions, handlers)
}

struct MotionLinkSpec;

impl super::spec::DomainLinkSpec for MotionLinkSpec {
	type Meta = MotionMetaRaw;
	type HandlerFn = MotionHandler;
	type Entry = MotionEntry;
	type Payload = MotionPayload;

	const WHAT: &'static str = "motion";
	const CANONICAL_PREFIX: &'static str = "xeno-registry::";

	fn common(meta: &Self::Meta) -> &crate::kdl::types::MetaCommonRaw {
		&meta.common
	}

	fn short_desc(meta: &Self::Meta) -> String {
		meta.common.name.clone()
	}

	fn build_payload(
		_meta: &Self::Meta,
		handler: Self::HandlerFn,
		_canonical_id: std::sync::Arc<str>,
	) -> Self::Payload {
		MotionPayload { handler }
	}
}
