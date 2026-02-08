use super::*;
use crate::core::{LinkedDef, LinkedMetaOwned, LinkedPayload, RegistryMeta, Symbol};
use crate::kdl::types::MotionsBlob;
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
	super::common::link_by_name(
		&metadata.motions,
		handlers,
		|m| &m.name,
		|h| h.name,
		|meta, handler| LinkedDef {
			meta: LinkedMetaOwned {
				id: format!("xeno-registry::{}", meta.name),
				name: meta.name.clone(),
				keys: meta.keys.clone(),
				description: meta.description.clone(),
				priority: 0,
				source: RegistrySource::Crate(handler.crate_name),
				required_caps: vec![],
				flags: 0,
				short_desc: None,
			},
			payload: MotionPayload {
				handler: handler.handler,
			},
		},
		"motion",
	)
}
