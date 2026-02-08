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
	let handler_map: HashMap<&str, &CommandHandlerStatic> = handlers.map(|h| (h.name, h)).collect();

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
