//! Gutter column registry.

use std::path::Path;

use ropey::RopeSlice;

pub use crate::themes::Color;
pub use crate::themes::theme::ThemeDef as Theme;

#[path = "compile/builtins.rs"]
pub mod builtins;
mod domain;
#[path = "exec/handler.rs"]
pub mod handler;
#[path = "compile/link.rs"]
pub mod link;
#[path = "compile/loader.rs"]
pub mod loader;
#[path = "exec/macros.rs"]
mod macros;
#[path = "contract/spec.rs"]
pub mod spec;

pub use builtins::register_builtins;
pub use domain::Gutters;
pub use handler::{GutterHandlerReg, GutterHandlerStatic};

/// Registers compiled gutters from the embedded spec.
pub fn register_compiled(db: &mut crate::db::builder::RegistryDbBuilder) {
	let spec = loader::load_gutters_spec();
	let handlers = inventory::iter::<handler::GutterHandlerReg>.into_iter().map(|r| r.0);

	let linked = link::link_gutters(&spec, handlers);

	for def in linked {
		db.push_domain::<Gutters>(GutterInput::Linked(def));
	}
}

use crate::core::index::{BuildCtx, BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	FrozenInterner, GutterId, RegistryBuilder, RegistryEntry, RegistryIndex, RegistryMeta, RegistryMetaStatic, RegistryRef, RegistrySource, RuntimeRegistry,
	Symbol, SymbolList,
};
// Re-export macros
pub use crate::gutter_handler;

/// Context passed to each gutter render closure (per-line).
pub struct GutterLineContext<'a> {
	pub line_idx: usize,
	pub total_lines: usize,
	pub cursor_line: usize,
	pub is_cursor_line: bool,
	pub is_continuation: bool,
	pub line_text: RopeSlice<'a>,
	pub path: Option<&'a Path>,
	pub annotations: &'a GutterAnnotations,
	pub theme: &'a Theme,
}

#[derive(Debug, Clone, Copy)]
pub struct GutterWidthContext {
	pub total_lines: usize,
	pub viewport_width: u16,
}

#[derive(Debug, Clone)]
pub struct GutterSegment {
	pub text: String,
	pub fg: Option<Color>,
	pub dim: bool,
}

#[derive(Debug, Clone)]
pub struct GutterCell {
	pub segments: Vec<GutterSegment>,
}

impl GutterCell {
	pub fn new(text: impl Into<String>, fg: Option<Color>, dim: bool) -> Self {
		Self {
			segments: vec![GutterSegment { text: text.into(), fg, dim }],
		}
	}

	pub fn styled(segments: Vec<GutterSegment>) -> Self {
		Self { segments }
	}
}

#[derive(Debug, Clone, Copy)]
pub enum GutterWidth {
	Fixed(u16),
	Dynamic(fn(&GutterWidthContext) -> u16),
}

#[derive(Debug, Clone, Default)]
pub struct GutterAnnotations {
	pub diagnostic_severity: u8,
	pub sign: Option<char>,
	pub diff_old_line: Option<u32>,
	pub diff_new_line: Option<u32>,
}

#[derive(Clone, Copy)]
pub struct GutterDef {
	pub meta: RegistryMetaStatic,
	pub default_enabled: bool,
	pub width: GutterWidth,
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
}

impl core::fmt::Debug for GutterDef {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("GutterDef")
			.field("name", &self.meta.name)
			.field("priority", &self.meta.priority)
			.field("default_enabled", &self.default_enabled)
			.finish()
	}
}

pub struct GutterEntry {
	pub meta: RegistryMeta,
	pub default_enabled: bool,
	pub width: GutterWidth,
	pub render: fn(&GutterLineContext) -> Option<GutterCell>,
}

crate::impl_registry_entry!(GutterEntry);

impl BuildEntry<GutterEntry> for GutterDef {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		RegistryMetaRef {
			id: self.meta.id,
			name: self.meta.name,
			keys: StrListRef::Static(self.meta.keys),
			description: self.meta.description,
			priority: self.meta.priority,
			source: self.meta.source,
			mutates_buffer: self.meta.mutates_buffer,
		}
	}

	fn short_desc_str(&self) -> &str {
		self.meta.name
	}

	fn collect_payload_strings<'b>(&'b self, _collector: &mut crate::core::index::StringCollector<'_, 'b>) {}

	fn build(&self, ctx: &mut dyn BuildCtx, key_pool: &mut Vec<Symbol>) -> GutterEntry {
		let meta = crate::core::index::meta_build::build_meta(ctx, key_pool, self.meta_ref(), []);

		GutterEntry {
			meta,
			default_enabled: self.default_enabled,
			width: self.width,
			render: self.render,
		}
	}
}

/// Unified input for gutter registration.
pub type GutterInput = crate::core::def_input::DefInput<GutterDef, crate::gutter::link::LinkedGutterDef>;

#[cfg(feature = "minimal")]
pub use crate::db::GUTTERS;

#[cfg(feature = "minimal")]
pub fn enabled_gutters() -> Vec<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS.snapshot_guard().iter_refs().filter(|g| g.default_enabled).collect()
}

#[cfg(feature = "minimal")]
pub fn find(name: &str) -> Option<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS.get(name)
}

#[cfg(feature = "minimal")]
pub fn all() -> Vec<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS.snapshot_guard().iter_refs().collect()
}

pub fn column_width(gutter: &GutterEntry, ctx: &GutterWidthContext) -> u16 {
	match gutter.width {
		GutterWidth::Fixed(w) => w,
		GutterWidth::Dynamic(f) => f(ctx),
	}
}

#[cfg(feature = "minimal")]
pub fn total_width(ctx: &GutterWidthContext) -> u16 {
	let width: u16 = enabled_gutters().iter().map(|g| column_width(g, ctx)).sum();
	if width > 0 { width + 1 } else { 0 }
}

#[cfg(feature = "minimal")]
pub fn column_widths(ctx: &GutterWidthContext) -> Vec<(u16, RegistryRef<GutterEntry, GutterId>)> {
	enabled_gutters().into_iter().map(|g| (column_width(&g, ctx), g)).collect()
}
