//! Statusline segment registry.

pub mod builtins;
mod macros;

use crate::core::index::{BuildEntry, RegistryMetaRef};
pub use crate::core::{
	CapabilitySet, FrozenInterner, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta,
	RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource, RuntimeRegistry,
	StatuslineId, Symbol, SymbolList,
};
// Re-export macros
pub use crate::segment;

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

impl crate::core::RegistryEntry for StatuslineSegmentDef {
	fn meta(&self) -> &RegistryMeta {
		panic!("Called meta() on static StatuslineSegmentDef")
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
			aliases: self.meta.aliases,
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

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		let meta = self.meta_ref();
		sink.push(meta.id);
		sink.push(meta.name);
		sink.push(meta.description);
		for &alias in meta.aliases {
			sink.push(alias);
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> StatuslineEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		for &alias in meta_ref.aliases {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		}
		let len = (alias_pool.len() as u32 - start) as u16;

		let meta = RegistryMeta {
			id: interner.get(meta_ref.id).expect("missing interned id"),
			name: interner.get(meta_ref.name).expect("missing interned name"),
			description: interner
				.get(meta_ref.description)
				.expect("missing interned description"),
			aliases: SymbolList { start, len },
			priority: meta_ref.priority,
			source: meta_ref.source,
			required_caps: CapabilitySet::from_iter(meta_ref.required_caps.iter().cloned()),
			flags: meta_ref.flags,
		};

		StatuslineEntry {
			meta,
			position: self.position,
			default_enabled: self.default_enabled,
			render: self.render,
		}
	}
}

#[cfg(feature = "db")]
pub use crate::db::STATUSLINE_SEGMENTS;

#[cfg(feature = "db")]
pub fn segments_for_position(
	position: SegmentPosition,
) -> Vec<RegistryRef<StatuslineEntry, StatuslineId>> {
	STATUSLINE_SEGMENTS
		.all()
		.into_iter()
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
	STATUSLINE_SEGMENTS.all()
}

pub use builtins::register_builtins;
