//! Links KDL metadata with Rust handler functions for actions and commands.
//!
//! At startup, `link_actions` and `link_commands` pair each `*MetaRaw` from
//! precompiled blobs with handler statics collected via `inventory`. The result
//! is `Linked*Def` types that implement `BuildEntry` for the registry builder.

use std::collections::{HashMap, HashSet};

use crate::core::index::{BuildEntry, RegistryMetaRef, StrListRef};
use crate::core::{FrozenInterner, RegistrySource, Symbol};

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
		pub keys: Vec<String>,
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
				keys: StrListRef::Owned(&self.keys),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
			sink.push(&self.short_desc);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> ActionEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: meta.keys.clone(),
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
		pub keys: Vec<String>,
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
				keys: StrListRef::Owned(&self.keys),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> CommandEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: meta.keys.clone(),
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

// ── Motions ───────────────────────────────────────────────────────────

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
		pub keys: Vec<String>,
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
				keys: StrListRef::Owned(&self.keys),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> MotionEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: meta.keys.clone(),
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
				keys: StrListRef::Owned(&[]),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> TextObjectEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: StrListRef::Owned(&[]),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> GutterEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: StrListRef::Owned(&[]),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> StatuslineEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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
				keys: StrListRef::Owned(&[]),
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
			crate::core::index::meta_build::collect_meta_strings(&self.meta_ref(), sink, []);
		}

		fn build(&self, interner: &FrozenInterner, key_pool: &mut Vec<Symbol>) -> HookEntry {
			let meta =
				crate::core::index::meta_build::build_meta(interner, key_pool, self.meta_ref(), []);

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

// ── Options ──────────────────────────────────────────────────────────

#[cfg(feature = "options")]
mod options_link {
	use super::*;
	use crate::kdl::types::OptionsBlob;
	use crate::options::def::{LinkedOptionDef, OptionScope};
	use crate::options::{OptionDefault, OptionType, OptionValidatorStatic, OptionValue};

	/// Links KDL option metadata with validator statics, producing `LinkedOptionDef`s.
	pub fn link_options(
		metadata: &OptionsBlob,
		validators: impl Iterator<Item = &'static OptionValidatorStatic>,
	) -> Vec<LinkedOptionDef> {
		let validator_map: HashMap<&str, &OptionValidatorStatic> =
			validators.map(|v| (v.name, v)).collect();

		let mut defs = Vec::new();

		for meta in &metadata.options {
			let id = format!("xeno-registry::{}", meta.name);

			let value_type = match meta.value_type.as_str() {
				"bool" => OptionType::Bool,
				"int" => OptionType::Int,
				"string" => OptionType::String,
				other => panic!("unknown option value-type: '{}'", other),
			};

			let scope = match meta.scope.as_str() {
				"global" => OptionScope::Global,
				"buffer" => OptionScope::Buffer,
				other => panic!("unknown option scope: '{}'", other),
			};

			let default = match value_type {
				OptionType::Bool => {
					let val = match meta.default.as_str() {
						"#true" | "true" => true,
						"#false" | "false" => false,
						other => panic!("invalid bool default: '{}'", other),
					};
					OptionDefault::Value(OptionValue::Bool(val))
				}
				OptionType::Int => {
					let val = meta
						.default
						.parse::<i64>()
						.unwrap_or_else(|_| panic!("invalid int default: '{}'", meta.default));
					OptionDefault::Value(OptionValue::Int(val))
				}
				OptionType::String => {
					OptionDefault::Value(OptionValue::String(meta.default.clone()))
				}
			};

			let validator = meta.validator.as_ref().map(|name| {
				validator_map
					.get(name.as_str())
					.map(|v| v.validator)
					.unwrap_or_else(|| {
						panic!(
							"KDL option '{}' references unknown validator '{}'",
							meta.name, name
						)
					})
			});

			defs.push(LinkedOptionDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				keys: meta.keys.clone(),
				priority: meta.priority,
				flags: meta.flags,
				kdl_key: meta.kdl_key.clone(),
				value_type,
				default,
				scope,
				validator,
				source: RegistrySource::Builtin,
			});
		}

		defs
	}
}

#[cfg(feature = "options")]
pub use options_link::*;

// ── Notifications ────────────────────────────────────────────────────

#[cfg(feature = "notifications")]
mod notifications_link {
	use std::time::Duration;

	use super::*;
	use crate::kdl::types::NotificationsBlob;
	use crate::notifications::def::LinkedNotificationDef;
	use crate::notifications::{AutoDismiss, Level};

	/// Links KDL notification metadata, producing `LinkedNotificationDef`s.
	pub fn link_notifications(metadata: &NotificationsBlob) -> Vec<LinkedNotificationDef> {
		let mut defs = Vec::new();

		for meta in &metadata.notifications {
			let id = format!("xeno-registry::{}", meta.name);

			let level = match meta.level.as_str() {
				"info" => Level::Info,
				"warn" => Level::Warn,
				"error" => Level::Error,
				"debug" => Level::Debug,
				"success" => Level::Success,
				other => panic!("unknown notification level: '{}'", other),
			};

			let auto_dismiss = match meta.auto_dismiss.as_str() {
				"never" => AutoDismiss::Never,
				"after" => {
					let ms = meta.dismiss_ms.unwrap_or(4000);
					AutoDismiss::After(Duration::from_millis(ms))
				}
				other => panic!("unknown auto-dismiss: '{}'", other),
			};

			defs.push(LinkedNotificationDef {
				id,
				name: meta.name.clone(),
				description: meta.description.clone(),
				keys: Vec::new(),
				priority: 0,
				flags: 0,
				level,
				auto_dismiss,
				source: RegistrySource::Builtin,
			});
		}

		defs
	}
}

#[cfg(feature = "notifications")]
pub use notifications_link::*;

// ── Themes ───────────────────────────────────────────────────────────

#[cfg(feature = "themes")]
mod themes_link {
	use std::str::FromStr;

	use super::*;
	use crate::kdl::types::{RawStyle, ThemesBlob};
	use crate::themes::theme::LinkedThemeDef;
	use crate::themes::{
		Color, ColorPair, ModeColors, Modifier, NotificationColors, PopupColors, SemanticColors,
		SyntaxStyle, SyntaxStyles, ThemeColors, ThemeVariant, UiColors,
	};

	pub fn link_themes(blob: &ThemesBlob) -> Vec<LinkedThemeDef> {
		blob.themes
			.iter()
			.map(|meta| {
				let id = format!("xeno-registry::{}", meta.name);

				let variant = match meta.variant.as_str() {
					"dark" => ThemeVariant::Dark,
					"light" => ThemeVariant::Light,
					other => panic!("Theme '{}' unknown variant: '{}'", meta.name, other),
				};

				let mut palette = HashMap::new();
				let mut pending = meta.palette.clone();
				let mut progress = true;
				while progress && !pending.is_empty() {
					progress = false;
					let mut resolved_in_pass = Vec::new();
					for (name, val) in &pending {
						if let Ok(color) = parse_color(val, &palette) {
							palette.insert(name.clone(), color);
							resolved_in_pass.push(name.clone());
							progress = true;
						}
					}
					for name in resolved_in_pass {
						pending.remove(&name);
					}
				}

				if !pending.is_empty() {
					panic!(
						"Theme '{}' has unresolved or cyclic palette references: {:?}",
						meta.name,
						pending.keys().collect::<Vec<_>>()
					);
				}

				let colors = ThemeColors {
					ui: build_ui_colors(&meta.ui, &palette, &meta.name),
					mode: build_mode_colors(&meta.mode, &palette, &meta.name),
					semantic: build_semantic_colors(&meta.semantic, &palette, &meta.name),
					popup: build_popup_colors(&meta.popup, &palette, &meta.name),
					notification: NotificationColors::INHERITED,
					syntax: build_syntax_styles(&meta.syntax, &palette, &meta.name),
				};

				LinkedThemeDef {
					id,
					name: meta.name.clone(),
					keys: meta.keys.clone(),
					description: meta.description.clone(),
					priority: meta.priority,
					variant,
					colors,
					source: RegistrySource::Builtin,
				}
			})
			.collect()
	}

	fn parse_color(s: &str, palette: &HashMap<String, Color>) -> Result<Color, String> {
		if let Some(name) = s.strip_prefix('$') {
			return palette
				.get(name)
				.copied()
				.ok_or_else(|| format!("unknown palette color: {name}"));
		}
		Color::from_str(s).map_err(|_| format!("invalid color: {s}"))
	}

	fn build_ui_colors(
		map: &HashMap<String, String>,
		palette: &HashMap<String, Color>,
		theme_name: &str,
	) -> UiColors {
		let get = |key: &str, default: Color| {
			map.get(key)
				.map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' UI error: {}: {}", theme_name, key, e)
					})
				})
				.unwrap_or(default)
		};
		let bg = get("bg", Color::Reset);
		UiColors {
			bg,
			fg: get("fg", Color::Reset),
			nontext_bg: get("nontext-bg", bg),
			gutter_fg: get("gutter-fg", Color::DarkGray),
			cursor_bg: get("cursor-bg", Color::White),
			cursor_fg: get("cursor-fg", Color::Black),
			cursorline_bg: get("cursorline-bg", Color::DarkGray),
			selection_bg: get("selection-bg", Color::Blue),
			selection_fg: get("selection-fg", Color::White),
			message_fg: get("message-fg", Color::Yellow),
			command_input_fg: get("command-input-fg", Color::White),
		}
	}

	fn build_mode_colors(
		map: &HashMap<String, String>,
		palette: &HashMap<String, Color>,
		theme_name: &str,
	) -> ModeColors {
		let get = |key: &str, default: Color| {
			map.get(key)
				.map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' mode error: {}: {}", theme_name, key, e)
					})
				})
				.unwrap_or(default)
		};
		ModeColors {
			normal: ColorPair::new(
				get("normal-bg", Color::Blue),
				get("normal-fg", Color::White),
			),
			insert: ColorPair::new(
				get("insert-bg", Color::Green),
				get("insert-fg", Color::Black),
			),
			prefix: ColorPair::new(
				get("prefix-bg", Color::Magenta),
				get("prefix-fg", Color::White),
			),
			command: ColorPair::new(
				get("command-bg", Color::Yellow),
				get("command-fg", Color::Black),
			),
		}
	}

	fn build_semantic_colors(
		map: &HashMap<String, String>,
		palette: &HashMap<String, Color>,
		theme_name: &str,
	) -> SemanticColors {
		let get = |key: &str, default: Color| {
			map.get(key)
				.map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' semantic error: {}: {}", theme_name, key, e)
					})
				})
				.unwrap_or(default)
		};
		SemanticColors {
			error: get("error", Color::Red),
			warning: get("warning", Color::Yellow),
			success: get("success", Color::Green),
			info: get("info", Color::Cyan),
			hint: get("hint", Color::DarkGray),
			dim: get("dim", Color::DarkGray),
			link: get("link", Color::Cyan),
			match_hl: get("match", Color::Green),
			accent: get("accent", Color::Cyan),
		}
	}

	fn build_popup_colors(
		map: &HashMap<String, String>,
		palette: &HashMap<String, Color>,
		theme_name: &str,
	) -> PopupColors {
		let get = |key: &str, default: Color| {
			map.get(key)
				.map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' popup error: {}: {}", theme_name, key, e)
					})
				})
				.unwrap_or(default)
		};
		PopupColors {
			bg: get("bg", Color::Reset),
			fg: get("fg", Color::Reset),
			border: get("border", Color::DarkGray),
			title: get("title", Color::Yellow),
		}
	}

	fn build_syntax_styles(
		map: &HashMap<String, RawStyle>,
		palette: &HashMap<String, Color>,
		theme_name: &str,
	) -> SyntaxStyles {
		let mut styles = SyntaxStyles::minimal();
		for (scope, raw) in map {
			let style = SyntaxStyle {
				fg: raw.fg.as_ref().map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' syntax fg error: {}: {}", theme_name, scope, e)
					})
				}),
				bg: raw.bg.as_ref().map(|s| {
					parse_color(s, palette).unwrap_or_else(|e| {
						panic!("Theme '{}' syntax bg error: {}: {}", theme_name, scope, e)
					})
				}),
				modifiers: raw
					.modifiers
					.as_ref()
					.map(|s| parse_modifiers(s, theme_name, scope))
					.unwrap_or(Modifier::empty()),
			};
			set_syntax_style(&mut styles, scope, style);
		}
		styles
	}

	pub(crate) fn parse_modifiers(s: &str, theme_name: &str, scope: &str) -> Modifier {
		let mut modifiers = Modifier::empty();
		for part in s.split('|').map(|s| s.trim()) {
			if part.is_empty() {
				continue;
			}
			match part.to_lowercase().as_str() {
				"bold" => modifiers.insert(Modifier::BOLD),
				"italic" => modifiers.insert(Modifier::ITALIC),
				"underlined" => modifiers.insert(Modifier::UNDERLINED),
				"reversed" => modifiers.insert(Modifier::REVERSED),
				"dim" => modifiers.insert(Modifier::DIM),
				"crossed-out" => modifiers.insert(Modifier::CROSSED_OUT),
				other => panic!(
					"Theme '{}' scope '{}' unknown modifier: '{}'",
					theme_name, scope, other
				),
			}
		}
		modifiers
	}

	fn set_syntax_style(styles: &mut SyntaxStyles, scope: &str, style: SyntaxStyle) {
		match scope {
			"attribute" => styles.attribute = style,
			"tag" => styles.tag = style,
			"namespace" => styles.namespace = style,
			"comment" => styles.comment = style,
			"comment.line" => styles.comment_line = style,
			"comment.block" => styles.comment_block = style,
			"comment.block.documentation" => styles.comment_block_documentation = style,
			"constant" => styles.constant = style,
			"constant.builtin" => styles.constant_builtin = style,
			"constant.builtin.boolean" => styles.constant_builtin_boolean = style,
			"constant.character" => styles.constant_character = style,
			"constant.character.escape" => styles.constant_character_escape = style,
			"constant.numeric" => styles.constant_numeric = style,
			"constant.numeric.integer" => styles.constant_numeric_integer = style,
			"constant.numeric.float" => styles.constant_numeric_float = style,
			"constructor" => styles.constructor = style,
			"function" => styles.function = style,
			"function.builtin" => styles.function_builtin = style,
			"function.method" => styles.function_method = style,
			"function.macro" => styles.function_macro = style,
			"function.special" => styles.function_special = style,
			"keyword" => styles.keyword = style,
			"keyword.control" => styles.keyword_control = style,
			"keyword.control.conditional" => styles.keyword_control_conditional = style,
			"keyword.control.repeat" => styles.keyword_control_repeat = style,
			"keyword.control.import" => styles.keyword_control_import = style,
			"keyword.control.return" => styles.keyword_control_return = style,
			"keyword.control.exception" => styles.keyword_control_exception = style,
			"keyword.operator" => styles.keyword_operator = style,
			"keyword.directive" => styles.keyword_directive = style,
			"keyword.function" => styles.keyword_function = style,
			"keyword.storage" => styles.keyword_storage = style,
			"keyword.storage.type" => styles.keyword_storage_type = style,
			"keyword.storage.modifier" => styles.keyword_storage_modifier = style,
			"label" => styles.label = style,
			"operator" => styles.operator = style,
			"punctuation" => styles.punctuation = style,
			"punctuation.bracket" => styles.punctuation_bracket = style,
			"punctuation.delimiter" => styles.punctuation_delimiter = style,
			"punctuation.special" => styles.punctuation_special = style,
			"string" => styles.string = style,
			"string.regexp" => styles.string_regexp = style,
			"string.special" => styles.string_special = style,
			"string.special.path" => styles.string_special_path = style,
			"string.special.url" => styles.string_special_url = style,
			"string.special.symbol" => styles.string_special_symbol = style,
			"type" => styles.r#type = style,
			"type.builtin" => styles.type_builtin = style,
			"type.parameter" => styles.type_parameter = style,
			"type.enum.variant" => styles.type_enum_variant = style,
			"variable" => styles.variable = style,
			"variable.builtin" => styles.variable_builtin = style,
			"variable.parameter" => styles.variable_parameter = style,
			"variable.other" => styles.variable_other = style,
			"variable.other.member" => styles.variable_other_member = style,
			"markup.heading" => styles.markup_heading = style,
			"markup.heading.1" => styles.markup_heading_1 = style,
			"markup.heading.2" => styles.markup_heading_2 = style,
			"markup.heading.3" => styles.markup_heading_3 = style,
			"markup.bold" => styles.markup_bold = style,
			"markup.italic" => styles.markup_italic = style,
			"markup.strikethrough" => styles.markup_strikethrough = style,
			"markup.link" => styles.markup_link = style,
			"markup.link.url" => styles.markup_link_url = style,
			"markup.link.text" => styles.markup_link_text = style,
			"markup.quote" => styles.markup_quote = style,
			"markup.raw" => styles.markup_raw = style,
			"markup.raw.inline" => styles.markup_raw_inline = style,
			"markup.raw.block" => styles.markup_raw_block = style,
			"markup.list" => styles.markup_list = style,
			"diff.plus" => styles.diff_plus = style,
			"diff.minus" => styles.diff_minus = style,
			"diff.delta" => styles.diff_delta = style,
			"special" => styles.special = style,
			_ => {}
		}
	}
}

#[cfg(feature = "themes")]
pub use themes_link::*;

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
	use super::*;
	use crate::kdl::loader::{
		load_action_metadata, load_command_metadata, load_gutter_metadata, load_hook_metadata,
		load_motion_metadata, load_option_metadata, load_statusline_metadata,
		load_text_object_metadata, load_theme_metadata,
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
	fn all_kdl_options_parse_and_validate() {
		let blob = load_option_metadata();
		let validators: Vec<&crate::options::OptionValidatorStatic> =
			inventory::iter::<crate::options::OptionValidatorReg>
				.into_iter()
				.map(|r| r.0)
				.collect();

		// This will panic if any option is invalid (type, default, scope, or unknown validator)
		options_link::link_options(&blob, validators.into_iter());
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

	// ── Theme linkage tests ───────────────────────────────────────────

	#[test]
	fn all_kdl_themes_parse_and_validate() {
		let blob = load_theme_metadata();
		// This will panic if any theme has invalid colors, unresolved palette refs, or bad modifiers
		link_themes(&blob);
	}

	#[test]
	fn default_theme_exists_in_kdl() {
		let blob = load_theme_metadata();
		let names: HashSet<&str> = blob.themes.iter().map(|t| t.name.as_str()).collect();
		assert!(
			names.contains("monokai"),
			"Default theme 'monokai' missing from KDL"
		);
	}

	#[test]
	fn modifier_parsing_works() {
		use crate::themes::Modifier;
		assert_eq!(
			themes_link::parse_modifiers("bold", "test", "test"),
			Modifier::BOLD
		);
		assert_eq!(
			themes_link::parse_modifiers("bold|italic", "test", "test"),
			Modifier::BOLD | Modifier::ITALIC
		);
		assert_eq!(
			themes_link::parse_modifiers("  bold | ITALIC  ", "test", "test"),
			Modifier::BOLD | Modifier::ITALIC
		);
	}

	#[test]
	#[should_panic(expected = "unknown modifier: 'invalid'")]
	fn modifier_parsing_panics_on_unknown() {
		themes_link::parse_modifiers("invalid", "test", "test");
	}
}
