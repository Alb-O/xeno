//! Statusline segment registry.

pub mod builtins;
pub mod handler;
pub mod link;
pub mod loader;
mod macros;
pub mod spec;

use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	CapabilitySet, FrozenInterner, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta,
	RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource, RuntimeRegistry,
	StatuslineId, Symbol, SymbolList,
};
// Re-export macros
pub use crate::segment_handler;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), crate::error::RegistryError> {
	register_compiled(db);
	Ok(())
}

/// Registers compiled statusline segments from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_statusline_spec();
	let handlers = inventory::iter::<handler::StatuslineHandlerReg>
		.into_iter()
		.map(|r| r.0);

	let linked = link::link_statusline(&spec, handlers);

	for def in linked {
		db.push_domain::<Statusline>(StatuslineInput::Linked(def));
	}
}

pub struct Statusline;

impl crate::db::domain::DomainSpec for Statusline {
	type Input = StatuslineInput;
	type Entry = StatuslineEntry;
	type Id = crate::core::StatuslineId;
	type StaticDef = StatuslineSegmentDef;
	type LinkedDef = link::LinkedStatuslineDef;
	const LABEL: &'static str = "statusline";

	fn static_to_input(def: &'static Self::StaticDef) -> Self::Input {
		StatuslineInput::Static(*def)
	}

	fn linked_to_input(def: Self::LinkedDef) -> Self::Input {
		StatuslineInput::Linked(def)
	}

	fn builder(
		db: &mut crate::db::builder::RegistryDbBuilder,
	) -> &mut crate::core::index::RegistryBuilder<Self::Input, Self::Entry, Self::Id> {
		&mut db.statusline
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentPosition {
	Left,
	Center,
	Right,
}

pub struct StatuslineContext<'a> {
	pub mode_name: &'a str,
	pub path: Option<&'a str>,
	pub modified: bool,
	pub readonly: bool,
	pub line: usize,
	pub col: usize,
	pub count: u32,
	pub total_lines: usize,
	pub file_type: Option<&'a str>,
	pub buffer_index: usize,
	pub buffer_count: usize,
	pub sync_role: Option<&'a str>,
	pub sync_status: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct RenderedSegment {
	pub text: String,
	pub style: SegmentStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SegmentStyle {
	#[default]
	Normal,
	Mode,
	Inverted,
	Dim,
	Warning,
	Error,
	Success,
}

#[derive(Clone, Copy)]
pub struct StatuslineSegmentDef {
	pub meta: RegistryMetaStatic,
	pub position: SegmentPosition,
	pub default_enabled: bool,
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
}

impl core::fmt::Debug for StatuslineSegmentDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("StatuslineSegmentDef")
			.field("name", &self.meta.name)
			.field("position", &self.position)
			.field("priority", &self.meta.priority)
			.finish()
	}
}

pub struct StatuslineEntry {
	pub meta: RegistryMeta,
	pub position: SegmentPosition,
	pub default_enabled: bool,
	pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
}

crate::impl_registry_entry!(StatuslineEntry);

impl BuildEntry<StatuslineEntry> for StatuslineSegmentDef {
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

	fn collect_payload_strings<'b>(
		&'b self,
		_collector: &mut crate::core::index::StringCollector<'_, 'b>,
	) {
	}

	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> StatuslineEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		StatuslineEntry {
			meta,
			position: self.position,
			default_enabled: self.default_enabled,
			render: self.render,
		}
	}
}

/// Unified input for statusline segment registration.
pub type StatuslineInput = crate::core::def_input::DefInput<
	StatuslineSegmentDef,
	crate::statusline::link::LinkedStatuslineDef,
>;

#[cfg(feature = "db")]
pub use crate::db::STATUSLINE_SEGMENTS;

#[cfg(feature = "db")]
pub fn segments_for_position(
	position: SegmentPosition,
) -> Vec<RegistryRef<StatuslineEntry, StatuslineId>> {
	STATUSLINE_SEGMENTS
		.snapshot_guard()
		.iter_refs()
		.filter(|s| s.position == position && s.default_enabled)
		.collect()
}

#[cfg(feature = "db")]
pub fn render_position(position: SegmentPosition, ctx: &StatuslineContext) -> Vec<RenderedSegment> {
	let mut segments = segments_for_position(position);
	segments.sort_by(|a, b| b.meta().priority.cmp(&a.meta().priority));
	segments
		.into_iter()
		.filter_map(|seg| (seg.render)(ctx))
		.collect()
}

#[cfg(feature = "db")]
pub fn find_segment(name: &str) -> Option<RegistryRef<StatuslineEntry, StatuslineId>> {
	STATUSLINE_SEGMENTS.get(name)
}

#[cfg(feature = "db")]
pub fn all_segments() -> Vec<RegistryRef<StatuslineEntry, StatuslineId>> {
	STATUSLINE_SEGMENTS.snapshot_guard().iter_refs().collect()
}
