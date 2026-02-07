//! Links KDL metadata with Rust handler functions for actions and commands.
//!
//! At startup, `link_actions` and `link_commands` pair each `*MetaRaw` from
//! precompiled blobs with handler statics collected via `inventory`. The result
//! is `Linked*Def` types that implement `BuildEntry` for the registry builder.

use std::collections::{HashMap, HashSet};

use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{
	CapabilitySet, FrozenInterner, RegistryMeta, RegistrySource, Symbol, SymbolList,
};

// ── Actions ──────────────────────────────────────────────────────────

#[cfg(feature = "actions")]
mod actions_link {
	use std::sync::Arc;

	use super::*;
	use crate::actions::def::ActionHandler;
	use crate::actions::entry::ActionEntry;
	use crate::actions::handler::ActionHandlerStatic;
	use crate::actions::{BindingMode, KeyBindingDef, KeyPrefixDef};
	use crate::core::capability::Capability;
	use crate::kdl::types::{ActionsBlob, KeyBindingRaw};

	/// An action definition assembled from KDL metadata + Rust handler.
	#[derive(Clone)]
	pub struct LinkedActionDef {
		/// Canonical ID: `"xeno-registry::{name}"`.
		pub id: String,
		/// Action name (linkage key).
		pub name: String,
		/// Human-readable description.
		pub description: String,
		/// Short description for which-key HUD.
		pub short_desc: String,
		/// Alternative lookup names.
		pub aliases: Vec<String>,
		/// Conflict resolution priority.
		pub priority: i16,
		/// Required capabilities.
		pub caps: Vec<Capability>,
		/// Behavior hint flags.
		pub flags: u32,
		/// Parsed key bindings.
		pub bindings: Vec<KeyBindingDef>,
		/// The handler function from Rust.
		pub handler: ActionHandler,
		/// Where this definition came from.
		pub source: RegistrySource,
	}

	impl BuildEntry<ActionEntry> for LinkedActionDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&self.aliases),
				description: &self.description,
				priority: self.priority,
				source: self.source,
				required_caps: &self.caps,
				flags: self.flags,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.short_desc
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
			for alias in &self.aliases {
				sink.push(alias);
			}
			sink.push(&self.short_desc);
		}

		fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> ActionEntry {
			let start = alias_pool.len() as u32;

			let mut unique_aliases = self.meta_ref().aliases.to_vec();
			unique_aliases.sort_unstable();
			unique_aliases.dedup();

			for alias in unique_aliases {
				alias_pool.push(interner.get(alias).expect("missing interned alias"));
			}
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: self.priority,
				source: self.source,
				required_caps: CapabilitySet::from_iter(self.caps.iter().cloned()),
				flags: self.flags,
			};

			ActionEntry {
				meta,
				short_desc: interner
					.get(&self.short_desc)
					.expect("missing interned short_desc"),
				handler: self.handler,
				bindings: Arc::from(self.bindings.as_slice()),
			}
		}
	}

	fn parse_binding_mode(mode: &str) -> BindingMode {
		match mode {
			"normal" => BindingMode::Normal,
			"insert" => BindingMode::Insert,
			"match" => BindingMode::Match,
			"space" => BindingMode::Space,
			other => panic!("unknown binding mode: '{}'", other),
		}
	}

	pub(crate) fn parse_capability(name: &str) -> Capability {
		match name {
			"Text" => Capability::Text,
			"Cursor" => Capability::Cursor,
			"Selection" => Capability::Selection,
			"Mode" => Capability::Mode,
			"Messaging" => Capability::Messaging,
			"Edit" => Capability::Edit,
			"Search" => Capability::Search,
			"Undo" => Capability::Undo,
			"FileOps" => Capability::FileOps,
			"Overlay" => Capability::Overlay,
			other => panic!("unknown capability: '{}'", other),
		}
	}

	pub(crate) fn parse_bindings(raw: &[KeyBindingRaw], action_id: Arc<str>) -> Vec<KeyBindingDef> {
		raw.iter()
			.map(|b| KeyBindingDef {
				mode: parse_binding_mode(&b.mode),
				keys: Arc::from(b.keys.as_str()),
				action: Arc::clone(&action_id),
				priority: 100,
			})
			.collect()
	}

	/// Links KDL metadata with handler statics, producing `LinkedActionDef`s.
	///
	/// Panics if any KDL action has no matching handler, or vice versa.
	pub fn link_actions(
		metadata: &ActionsBlob,
		handlers: impl Iterator<Item = &'static ActionHandlerStatic>,
	) -> Vec<LinkedActionDef> {
		let handler_map: HashMap<&str, &ActionHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.actions {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL action '{}' has no matching action_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);
			let action_id: Arc<str> = Arc::from(id.as_str());
			let short_desc = meta
				.short_desc
				.clone()
				.unwrap_or_else(|| meta.description.clone());
			let caps = meta.caps.iter().map(|c| parse_capability(c)).collect();
			let bindings = parse_bindings(&meta.bindings, action_id);

			defs.push(LinkedActionDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				short_desc,
				aliases: meta.aliases.clone(),
				priority: meta.priority,
				caps,
				flags: meta.flags,
				bindings,
				handler: handler.handler,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"action_handler!({}) has no matching entry in actions.kdl",
					name
				);
			}
		}

		defs
	}

	/// Parses prefix data from the blob into `KeyPrefixDef`s.
	pub fn link_prefixes(metadata: &ActionsBlob) -> Vec<KeyPrefixDef> {
		metadata
			.prefixes
			.iter()
			.map(|p| KeyPrefixDef {
				mode: parse_binding_mode(&p.mode),
				keys: Arc::from(p.keys.as_str()),
				description: Arc::from(p.description.as_str()),
			})
			.collect()
	}
}

#[cfg(feature = "actions")]
pub use actions_link::*;

// ── Commands ──────────────────────────────────────────────────────────

#[cfg(feature = "commands")]
mod commands_link {
	use super::*;
	use crate::commands::def::CommandHandler;
	use crate::commands::entry::CommandEntry;
	use crate::commands::handler::CommandHandlerStatic;
	use crate::kdl::types::CommandsBlob;

	/// A command definition assembled from KDL metadata + Rust handler.
	#[derive(Clone)]
	pub struct LinkedCommandDef {
		/// Canonical ID: `"xeno-registry::{name}"`.
		pub id: String,
		/// Command name (linkage key).
		pub name: String,
		/// Human-readable description.
		pub description: String,
		/// Alternative lookup names (e.g., `"q"` for `"quit"`).
		pub aliases: Vec<String>,
		/// The async handler function from Rust.
		pub handler: CommandHandler,
		/// Where this definition came from.
		pub source: RegistrySource,
	}

	impl BuildEntry<CommandEntry> for LinkedCommandDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&self.aliases),
				description: &self.description,
				priority: 0,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
			for alias in &self.aliases {
				sink.push(alias);
			}
		}

		fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> CommandEntry {
			let start = alias_pool.len() as u32;

			let mut unique_aliases = self.meta_ref().aliases.to_vec();
			unique_aliases.sort_unstable();
			unique_aliases.dedup();

			for alias in unique_aliases {
				alias_pool.push(interner.get(alias).expect("missing interned alias"));
			}
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: 0,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

			CommandEntry {
				meta,
				handler: self.handler,
				user_data: None,
			}
		}
	}

	/// Links KDL command metadata with handler statics, producing `LinkedCommandDef`s.
	///
	/// Panics if any KDL command has no matching handler, or vice versa.
	pub fn link_commands(
		metadata: &CommandsBlob,
		handlers: impl Iterator<Item = &'static CommandHandlerStatic>,
	) -> Vec<LinkedCommandDef> {
		let handler_map: HashMap<&str, &CommandHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.commands {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL command '{}' has no matching command_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);

			defs.push(LinkedCommandDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				aliases: meta.aliases.clone(),
				handler: handler.handler,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"command_handler!({}) has no matching entry in commands.kdl",
					name
				);
			}
		}

		defs
	}
}

#[cfg(feature = "commands")]
pub use commands_link::*;

// ── Motions ──────────────────────────────────────────────────────────

#[cfg(feature = "motions")]
mod motions_link {
	use super::*;
	use crate::kdl::types::MotionsBlob;
	use crate::motions::handler::MotionHandlerStatic;
	use crate::motions::{MotionEntry, MotionHandler};

	/// A motion definition assembled from KDL metadata + Rust handler.
	#[derive(Clone)]
	pub struct LinkedMotionDef {
		/// Canonical ID: `"xeno-registry::{name}"`.
		pub id: String,
		/// Motion name (linkage key).
		pub name: String,
		/// Human-readable description.
		pub description: String,
		/// Alternative lookup names.
		pub aliases: Vec<String>,
		/// The handler function from Rust.
		pub handler: MotionHandler,
		/// Where this definition came from.
		pub source: RegistrySource,
	}

	impl BuildEntry<MotionEntry> for LinkedMotionDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&self.aliases),
				description: &self.description,
				priority: 0,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
			for alias in &self.aliases {
				sink.push(alias);
			}
		}

		fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> MotionEntry {
			let start = alias_pool.len() as u32;

			let mut unique_aliases = self.meta_ref().aliases.to_vec();
			unique_aliases.sort_unstable();
			unique_aliases.dedup();

			for alias in unique_aliases {
				alias_pool.push(interner.get(alias).expect("missing interned alias"));
			}
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: 0,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

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
		let handler_map: HashMap<&str, &MotionHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.motions {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL motion '{}' has no matching motion_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);

			defs.push(LinkedMotionDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				aliases: meta.aliases.clone(),
				handler: handler.handler,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"motion_handler!({}) has no matching entry in motions.kdl",
					name
				);
			}
		}

		defs
	}
}

#[cfg(feature = "motions")]
pub use motions_link::*;

// ── Text Objects ─────────────────────────────────────────────────────

#[cfg(feature = "textobj")]
mod textobj_link {
	use std::sync::Arc;

	use super::*;
	use crate::kdl::types::TextObjectsBlob;
	use crate::textobj::handler::TextObjectHandlerStatic;
	use crate::textobj::{TextObjectEntry, TextObjectHandler};

	/// A text object definition assembled from KDL metadata + Rust handlers.
	#[derive(Clone)]
	pub struct LinkedTextObjectDef {
		/// Canonical ID: `"xeno-registry::{name}"`.
		pub id: String,
		/// Text object name (linkage key).
		pub name: String,
		/// Human-readable description.
		pub description: String,
		/// Primary trigger character.
		pub trigger: char,
		/// Alternate trigger characters.
		pub alt_triggers: Vec<char>,
		/// Inner selection handler from Rust.
		pub inner: TextObjectHandler,
		/// Around selection handler from Rust.
		pub around: TextObjectHandler,
		/// Where this definition came from.
		pub source: RegistrySource,
	}

	impl BuildEntry<TextObjectEntry> for LinkedTextObjectDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&[]),
				description: &self.description,
				priority: 0,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
		}

		fn build(
			&self,
			interner: &FrozenInterner,
			alias_pool: &mut Vec<Symbol>,
		) -> TextObjectEntry {
			let start = alias_pool.len() as u32;
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: 0,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

			TextObjectEntry {
				meta,
				trigger: self.trigger,
				alt_triggers: Arc::from(self.alt_triggers.as_slice()),
				inner: self.inner,
				around: self.around,
			}
		}
	}

	/// Parses a single-character trigger string into a `char`.
	fn parse_trigger(s: &str, name: &str) -> char {
		let mut chars = s.chars();
		let c = chars
			.next()
			.unwrap_or_else(|| panic!("text object '{}' has empty trigger", name));
		assert!(
			chars.next().is_none(),
			"text object '{}' trigger '{}' is not a single character",
			name,
			s
		);
		c
	}

	/// Links KDL text object metadata with handler statics, producing `LinkedTextObjectDef`s.
	///
	/// Panics if any KDL text object has no matching handler, or vice versa.
	pub fn link_text_objects(
		metadata: &TextObjectsBlob,
		handlers: impl Iterator<Item = &'static TextObjectHandlerStatic>,
	) -> Vec<LinkedTextObjectDef> {
		let handler_map: HashMap<&str, &TextObjectHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.text_objects {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL text object '{}' has no matching text_object_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);
			let trigger = parse_trigger(&meta.trigger, &meta.name);
			let alt_triggers: Vec<char> = meta
				.alt_triggers
				.iter()
				.map(|s| parse_trigger(s, &meta.name))
				.collect();

			defs.push(LinkedTextObjectDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				trigger,
				alt_triggers,
				inner: handler.inner,
				around: handler.around,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"text_object_handler!({}) has no matching entry in text_objects.kdl",
					name
				);
			}
		}

		defs
	}
}

#[cfg(feature = "textobj")]
pub use textobj_link::*;

// ── Gutters ──────────────────────────────────────────────────────────

#[cfg(feature = "gutter")]
mod gutters_link {
	use super::*;
	use crate::gutter::handler::GutterHandlerStatic;
	use crate::gutter::{GutterCell, GutterEntry, GutterLineContext, GutterWidth};
	use crate::kdl::types::GuttersBlob;

	/// A gutter definition assembled from KDL metadata + Rust handlers.
	#[derive(Clone)]
	pub struct LinkedGutterDef {
		pub id: String,
		pub name: String,
		pub description: String,
		pub priority: i16,
		pub default_enabled: bool,
		pub width: GutterWidth,
		pub render: fn(&GutterLineContext) -> Option<GutterCell>,
		pub source: RegistrySource,
	}

	impl BuildEntry<GutterEntry> for LinkedGutterDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&[]),
				description: &self.description,
				priority: self.priority,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
		}

		fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> GutterEntry {
			let start = alias_pool.len() as u32;
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: self.priority,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

			GutterEntry {
				meta,
				default_enabled: self.default_enabled,
				width: self.width,
				render: self.render,
			}
		}
	}

	/// Links KDL gutter metadata with handler statics.
	pub fn link_gutters(
		metadata: &GuttersBlob,
		handlers: impl Iterator<Item = &'static GutterHandlerStatic>,
	) -> Vec<LinkedGutterDef> {
		let handler_map: HashMap<&str, &GutterHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.gutters {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL gutter '{}' has no matching gutter_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);

			defs.push(LinkedGutterDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				priority: meta.priority,
				default_enabled: meta.enabled,
				width: handler.width,
				render: handler.render,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"gutter_handler!({}) has no matching entry in gutters.kdl",
					name
				);
			}
		}

		defs
	}
}

#[cfg(feature = "gutter")]
pub use gutters_link::*;

// ── Statusline ───────────────────────────────────────────────────────

#[cfg(feature = "statusline")]
mod statusline_link {
	use super::*;
	use crate::kdl::types::StatuslineBlob;
	use crate::statusline::handler::StatuslineHandlerStatic;
	use crate::statusline::{RenderedSegment, SegmentPosition, StatuslineContext, StatuslineEntry};

	/// A statusline segment definition assembled from KDL metadata + Rust handler.
	#[derive(Clone)]
	pub struct LinkedStatuslineDef {
		pub id: String,
		pub name: String,
		pub description: String,
		pub priority: i16,
		pub position: SegmentPosition,
		pub default_enabled: bool,
		pub render: fn(&StatuslineContext) -> Option<RenderedSegment>,
		pub source: RegistrySource,
	}

	fn parse_position(s: &str, name: &str) -> SegmentPosition {
		match s {
			"left" => SegmentPosition::Left,
			"center" => SegmentPosition::Center,
			"right" => SegmentPosition::Right,
			other => panic!("unknown position '{}' for segment '{}'", other, name),
		}
	}

	impl BuildEntry<StatuslineEntry> for LinkedStatuslineDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&[]),
				description: &self.description,
				priority: self.priority,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
		}

		fn build(
			&self,
			interner: &FrozenInterner,
			alias_pool: &mut Vec<Symbol>,
		) -> StatuslineEntry {
			let start = alias_pool.len() as u32;
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: self.priority,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

			StatuslineEntry {
				meta,
				position: self.position,
				default_enabled: self.default_enabled,
				render: self.render,
			}
		}
	}

	/// Links KDL statusline metadata with handler statics.
	pub fn link_statusline(
		metadata: &StatuslineBlob,
		handlers: impl Iterator<Item = &'static StatuslineHandlerStatic>,
	) -> Vec<LinkedStatuslineDef> {
		let handler_map: HashMap<&str, &StatuslineHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.segments {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL segment '{}' has no matching segment_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);
			let position = parse_position(&meta.position, &meta.name);

			defs.push(LinkedStatuslineDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				priority: meta.priority,
				position,
				default_enabled: true,
				render: handler.render,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!(
					"segment_handler!({}) has no matching entry in statusline.kdl",
					name
				);
			}
		}

		defs
	}
}

#[cfg(feature = "statusline")]
pub use statusline_link::*;

// ── Hooks ────────────────────────────────────────────────────────────

#[cfg(feature = "hooks")]
mod hooks_link {
	use super::*;
	use crate::HookEvent;
	use crate::hooks::handler::HookHandlerStatic;
	use crate::hooks::{HookEntry, HookHandler, HookMutability, HookPriority};
	use crate::kdl::types::HooksBlob;

	/// A hook definition assembled from KDL metadata + Rust handler.
	#[derive(Clone)]
	pub struct LinkedHookDef {
		pub id: String,
		pub name: String,
		pub description: String,
		pub priority: i16,
		pub event: HookEvent,
		pub mutability: HookMutability,
		pub execution_priority: HookPriority,
		pub handler: HookHandler,
		pub source: RegistrySource,
	}

	impl BuildEntry<HookEntry> for LinkedHookDef {
		fn meta_ref(&self) -> RegistryMetaRef<'_> {
			RegistryMetaRef {
				id: &self.id,
				name: &self.name,
				aliases: StrListRef::Owned(&[]),
				description: &self.description,
				priority: self.priority,
				source: self.source,
				required_caps: &[],
				flags: 0,
			}
		}

		fn short_desc_str(&self) -> &str {
			&self.name
		}

		fn collect_strings<'a>(&'a self, sink: &mut Vec<&'a str>) {
			sink.push(&self.id);
			sink.push(&self.name);
			sink.push(&self.description);
		}

		fn build(&self, interner: &FrozenInterner, alias_pool: &mut Vec<Symbol>) -> HookEntry {
			let start = alias_pool.len() as u32;
			let len = (alias_pool.len() as u32 - start) as u16;

			let meta = RegistryMeta {
				id: interner.get(&self.id).expect("missing interned id"),
				name: interner.get(&self.name).expect("missing interned name"),
				description: interner
					.get(&self.description)
					.expect("missing interned description"),
				aliases: SymbolList { start, len },
				priority: self.priority,
				source: self.source,
				required_caps: CapabilitySet::empty(),
				flags: 0,
			};

			HookEntry {
				meta,
				event: self.event,
				mutability: self.mutability,
				execution_priority: self.execution_priority,
				handler: self.handler,
			}
		}
	}

	/// Links KDL hook metadata with handler statics.
	pub fn link_hooks(
		metadata: &HooksBlob,
		handlers: impl Iterator<Item = &'static HookHandlerStatic>,
	) -> Vec<LinkedHookDef> {
		let handler_map: HashMap<&str, &HookHandlerStatic> =
			handlers.map(|h| (h.name, h)).collect();

		let mut defs = Vec::new();
		let mut used_handlers = HashSet::new();

		for meta in &metadata.hooks {
			let handler = handler_map.get(meta.name.as_str()).unwrap_or_else(|| {
				panic!(
					"KDL hook '{}' has no matching hook_handler!() in Rust",
					meta.name
				)
			});
			used_handlers.insert(meta.name.as_str());

			let id = format!("xeno-registry::{}", meta.name);

			defs.push(LinkedHookDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				priority: meta.priority,
				event: handler.event,
				mutability: handler.mutability,
				execution_priority: handler.execution_priority,
				handler: handler.handler,
				source: RegistrySource::Crate(handler.crate_name),
			});
		}

		for name in handler_map.keys() {
			if !used_handlers.contains(name) {
				panic!("hook_handler!({}) has no matching entry in hooks.kdl", name);
			}
		}

		defs
	}
}

#[cfg(feature = "hooks")]
pub use hooks_link::*;

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;
	use crate::kdl::loader::{
		load_action_metadata, load_command_metadata, load_gutter_metadata, load_hook_metadata,
		load_motion_metadata, load_option_metadata, load_statusline_metadata,
		load_text_object_metadata,
	};

	// ── Action linkage tests ──────────────────────────────────────────

	#[test]
	fn all_kdl_actions_have_handlers() {
		use crate::actions::handler::ActionHandlerStatic;
		let blob = load_action_metadata();
		let handlers: Vec<&ActionHandlerStatic> =
			inventory::iter::<crate::actions::ActionHandlerReg>
				.into_iter()
				.map(|r| r.0)
				.collect();
		let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

		for action in &blob.actions {
			assert!(
				handler_names.contains(action.name.as_str()),
				"KDL action '{}' has no handler",
				action.name
			);
		}
	}

	#[test]
	fn all_handlers_have_kdl_entries() {
		let blob = load_action_metadata();
		let kdl_names: HashSet<&str> = blob.actions.iter().map(|a| a.name.as_str()).collect();

		for reg in inventory::iter::<crate::actions::ActionHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"handler '{}' has no KDL entry",
				reg.0.name
			);
		}
	}

	#[test]
	fn bindings_parse_correctly() {
		use std::sync::Arc;

		use crate::actions::BindingMode;
		use crate::kdl::types::KeyBindingRaw;

		let raw = vec![
			KeyBindingRaw {
				mode: "normal".into(),
				keys: "g g".into(),
			},
			KeyBindingRaw {
				mode: "insert".into(),
				keys: "esc".into(),
			},
		];
		let bindings = actions_link::parse_bindings(&raw, Arc::from("test::action"));
		assert_eq!(bindings.len(), 2);
		assert_eq!(bindings[0].mode, BindingMode::Normal);
		assert_eq!(&*bindings[0].keys, "g g");
		assert_eq!(bindings[1].mode, BindingMode::Insert);
		assert_eq!(&*bindings[1].keys, "esc");
	}

	#[test]
	fn capabilities_parse_correctly() {
		use crate::core::capability::Capability;

		assert_eq!(actions_link::parse_capability("Text"), Capability::Text);
		assert_eq!(actions_link::parse_capability("Edit"), Capability::Edit);
		assert_eq!(actions_link::parse_capability("Cursor"), Capability::Cursor);
		assert_eq!(
			actions_link::parse_capability("Selection"),
			Capability::Selection
		);
		assert_eq!(actions_link::parse_capability("Mode"), Capability::Mode);
		assert_eq!(
			actions_link::parse_capability("Messaging"),
			Capability::Messaging
		);
		assert_eq!(actions_link::parse_capability("Search"), Capability::Search);
		assert_eq!(actions_link::parse_capability("Undo"), Capability::Undo);
		assert_eq!(
			actions_link::parse_capability("FileOps"),
			Capability::FileOps
		);
		assert_eq!(
			actions_link::parse_capability("Overlay"),
			Capability::Overlay
		);
	}

	// ── Command linkage tests ─────────────────────────────────────────

	#[test]
	fn all_kdl_commands_have_handlers() {
		use crate::commands::handler::CommandHandlerStatic;
		let blob = load_command_metadata();
		let handlers: Vec<&CommandHandlerStatic> =
			inventory::iter::<crate::commands::CommandHandlerReg>
				.into_iter()
				.map(|r| r.0)
				.collect();
		let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

		for cmd in &blob.commands {
			assert!(
				handler_names.contains(cmd.name.as_str()),
				"KDL command '{}' has no handler",
				cmd.name
			);
		}
	}

	#[test]
	fn all_command_handlers_have_kdl_entries() {
		let blob = load_command_metadata();
		let kdl_names: HashSet<&str> = blob.commands.iter().map(|c| c.name.as_str()).collect();

		for reg in inventory::iter::<crate::commands::CommandHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"command_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}

	// ── Motion linkage tests ──────────────────────────────────────────

	#[test]
	fn all_kdl_motions_have_handlers() {
		use crate::motions::handler::MotionHandlerStatic;
		let blob = load_motion_metadata();
		let handlers: Vec<&MotionHandlerStatic> =
			inventory::iter::<crate::motions::MotionHandlerReg>
				.into_iter()
				.map(|r| r.0)
				.collect();
		let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

		for motion in &blob.motions {
			assert!(
				handler_names.contains(motion.name.as_str()),
				"KDL motion '{}' has no handler",
				motion.name
			);
		}
	}

	#[test]
	fn all_motion_handlers_have_kdl_entries() {
		let blob = load_motion_metadata();
		let kdl_names: HashSet<&str> = blob.motions.iter().map(|m| m.name.as_str()).collect();

		for reg in inventory::iter::<crate::motions::MotionHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"motion_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}

	// ── Text object linkage tests ────────────────────────────────────

	#[test]
	fn all_kdl_text_objects_have_handlers() {
		use crate::textobj::handler::TextObjectHandlerStatic;
		let blob = load_text_object_metadata();
		let handlers: Vec<&TextObjectHandlerStatic> =
			inventory::iter::<crate::textobj::TextObjectHandlerReg>
				.into_iter()
				.map(|r| r.0)
				.collect();
		let handler_names: HashSet<&str> = handlers.iter().map(|h| h.name).collect();

		for obj in &blob.text_objects {
			assert!(
				handler_names.contains(obj.name.as_str()),
				"KDL text object '{}' has no handler",
				obj.name
			);
		}
	}

	#[test]
	fn all_text_object_handlers_have_kdl_entries() {
		let blob = load_text_object_metadata();
		let kdl_names: HashSet<&str> = blob.text_objects.iter().map(|t| t.name.as_str()).collect();

		for reg in inventory::iter::<crate::textobj::TextObjectHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"text_object_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}

	// ── Option metadata validation ───────────────────────────────────

	#[test]
	fn all_static_options_have_kdl_entries() {
		let blob = load_option_metadata();
		let kdl_keys: HashSet<&str> = blob.options.iter().map(|o| o.kdl_key.as_str()).collect();

		for reg in inventory::iter::<crate::options::OptionReg> {
			assert!(
				kdl_keys.contains(reg.0.kdl_key),
				"static option '{}' (kdl_key='{}') has no KDL entry",
				reg.0.meta.name,
				reg.0.kdl_key
			);
		}
	}

	#[test]
	fn all_kdl_options_have_static_defs() {
		let blob = load_option_metadata();
		let static_keys: HashSet<&str> = inventory::iter::<crate::options::OptionReg>
			.into_iter()
			.map(|r| r.0.kdl_key)
			.collect();

		for opt in &blob.options {
			assert!(
				static_keys.contains(opt.kdl_key.as_str()),
				"KDL option '{}' (kdl_key='{}') has no static def",
				opt.name,
				opt.kdl_key
			);
		}
	}

	#[test]
	fn option_kdl_keys_match() {
		use std::collections::HashMap;

		let blob = load_option_metadata();
		let kdl_map: HashMap<&str, &crate::kdl::types::OptionMetaRaw> = blob
			.options
			.iter()
			.map(|o| (o.kdl_key.as_str(), o))
			.collect();

		for reg in inventory::iter::<crate::options::OptionReg> {
			let def = reg.0;
			let kdl = kdl_map.get(def.kdl_key).unwrap_or_else(|| {
				panic!(
					"static option '{}' not found in KDL by kdl_key",
					def.meta.name
				)
			});
			assert_eq!(
				def.kdl_key, kdl.kdl_key,
				"kdl_key mismatch for option '{}'",
				def.meta.name
			);
		}
	}

	// ── Gutter linkage tests ─────────────────────────────────────────

	#[test]
	fn all_kdl_gutters_have_handlers() {
		let blob = load_gutter_metadata();
		let handler_names: HashSet<&str> = inventory::iter::<crate::gutter::GutterHandlerReg>
			.into_iter()
			.map(|r| r.0.name)
			.collect();

		for gutter in &blob.gutters {
			assert!(
				handler_names.contains(gutter.name.as_str()),
				"KDL gutter '{}' has no handler",
				gutter.name
			);
		}
	}

	#[test]
	fn all_gutter_handlers_have_kdl_entries() {
		let blob = load_gutter_metadata();
		let kdl_names: HashSet<&str> = blob.gutters.iter().map(|g| g.name.as_str()).collect();

		for reg in inventory::iter::<crate::gutter::GutterHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"gutter_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}

	// ── Statusline linkage tests ─────────────────────────────────────

	#[test]
	fn all_kdl_segments_have_handlers() {
		let blob = load_statusline_metadata();
		let handler_names: HashSet<&str> =
			inventory::iter::<crate::statusline::handler::StatuslineHandlerReg>
				.into_iter()
				.map(|r| r.0.name)
				.collect();

		for seg in &blob.segments {
			assert!(
				handler_names.contains(seg.name.as_str()),
				"KDL segment '{}' has no handler",
				seg.name
			);
		}
	}

	#[test]
	fn all_segment_handlers_have_kdl_entries() {
		let blob = load_statusline_metadata();
		let kdl_names: HashSet<&str> = blob.segments.iter().map(|s| s.name.as_str()).collect();

		for reg in inventory::iter::<crate::statusline::handler::StatuslineHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"segment_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}

	// ── Hook linkage tests ───────────────────────────────────────────

	#[test]
	fn all_kdl_hooks_have_handlers() {
		let blob = load_hook_metadata();
		let handler_names: HashSet<&str> = inventory::iter::<crate::hooks::HookHandlerReg>
			.into_iter()
			.map(|r| r.0.name)
			.collect();

		for hook in &blob.hooks {
			assert!(
				handler_names.contains(hook.name.as_str()),
				"KDL hook '{}' has no handler",
				hook.name
			);
		}
	}

	#[test]
	fn all_hook_handlers_have_kdl_entries() {
		let blob = load_hook_metadata();
		let kdl_names: HashSet<&str> = blob.hooks.iter().map(|h| h.name.as_str()).collect();

		for reg in inventory::iter::<crate::hooks::HookHandlerReg> {
			assert!(
				kdl_names.contains(reg.0.name),
				"hook_handler!({}) has no KDL entry",
				reg.0.name
			);
		}
	}
}
