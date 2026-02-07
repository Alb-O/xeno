//! Gutter column registry.

use std::path::Path;

use ropey::RopeSlice;

pub use crate::themes::Color;
pub use crate::themes::theme::ThemeDef as Theme;

pub mod builtins;
pub mod handler;
mod macros;

pub use builtins::register_builtins;
pub use handler::{GutterHandlerReg, GutterHandlerStatic};

use crate::error::RegistryError;

pub fn register_plugin(
	db: &mut crate::db::builder::RegistryDbBuilder,
) -> Result<(), RegistryError> {
	register_builtins(db);
	Ok(())
}

use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
pub use crate::core::{
	CapabilitySet, FrozenInterner, GutterId, RegistryBuilder, RegistryEntry, RegistryIndex,
	RegistryMeta, RegistryMetaStatic, RegistryMetadata, RegistryRef, RegistrySource,
	RuntimeRegistry, Symbol, SymbolList,
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
			segments: vec![GutterSegment {
				text: text.into(),
				fg,
				dim,
			}],
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
			aliases: StrListRef::Static(self.meta.aliases),
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
		meta.aliases.for_each(|a| sink.push(a));
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> GutterEntry {
		let meta_ref = self.meta_ref();
		let start = alias_pool.len() as u32;
		meta_ref.aliases.for_each(|alias| {
			alias_pool.push(interner.get(alias).expect("missing interned alias"));
		});
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

		GutterEntry {
			meta,
			default_enabled: self.default_enabled,
			width: self.width,
			render: self.render,
		}
	}
}

/// Unified input for gutter registration.
pub enum GutterInput {
	Static(GutterDef),
	Linked(crate::kdl::link::LinkedGutterDef),
}

impl BuildEntry<GutterEntry> for GutterInput {
	fn meta_ref(&self) -> RegistryMetaRef<'_> {
		match self {
			Self::Static(d) => d.meta_ref(),
			Self::Linked(d) => d.meta_ref(),
		}
	}

	fn short_desc_str(&self) -> &str {
		match self {
			Self::Static(d) => d.short_desc_str(),
			Self::Linked(d) => d.short_desc_str(),
		}
	}

	fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
		match self {
			Self::Static(d) => d.collect_strings(sink),
			Self::Linked(d) => d.collect_strings(sink),
		}
	}

	fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> GutterEntry {
		match self {
			Self::Static(d) => d.build(interner, alias_pool),
			Self::Linked(d) => d.build(interner, alias_pool),
		}
	}
}

#[cfg(feature = "db")]
pub use crate::db::GUTTERS;

#[cfg(feature = "db")]
pub fn enabled_gutters() -> Vec<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS
		.all()
		.into_iter()
		.filter(|g| g.default_enabled)
		.collect()
}

#[cfg(feature = "db")]
pub fn find(name: &str) -> Option<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS.get(name)
}

#[cfg(feature = "db")]
pub fn all() -> Vec<RegistryRef<GutterEntry, GutterId>> {
	GUTTERS.all()
}

pub fn column_width(gutter: &GutterEntry, ctx: &GutterWidthContext) -> u16 {
	match gutter.width {
		GutterWidth::Fixed(w) => w,
		GutterWidth::Dynamic(f) => f(ctx),
	}
}

#[cfg(feature = "db")]
pub fn total_width(ctx: &GutterWidthContext) -> u16 {
	let width: u16 = enabled_gutters().iter().map(|g| column_width(g, ctx)).sum();
	if width > 0 { width + 1 } else { 0 }
}

#[cfg(feature = "db")]
pub fn column_widths(ctx: &GutterWidthContext) -> Vec<(u16, RegistryRef<GutterEntry, GutterId>)> {
	enabled_gutters()
		.into_iter()
		.map(|g| (column_width(&g, ctx), g))
		.collect()
}
