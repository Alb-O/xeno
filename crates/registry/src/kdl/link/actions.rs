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

pub(crate) fn parse_binding_mode(mode: &str) -> BindingMode {
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
	let handler_map: HashMap<&str, &ActionHandlerStatic> = handlers.map(|h| (h.name, h)).collect();

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
