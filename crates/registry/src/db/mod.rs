use std::sync::{LazyLock, OnceLock};

pub use crate::core::{ActionId, RegistryIndex, RuntimeRegistry};

pub mod builder;
pub mod builtins;
pub mod index;
#[cfg(feature = "keymap")]
pub mod keymap_registry;
pub mod plugin;

use crate::actions::{ActionDef, KeyPrefixDef};
use crate::commands::CommandDef;
#[cfg(feature = "keymap")]
use crate::db::keymap_registry::KeymapRegistry;
use crate::gutter::GutterDef;
use crate::motions::MotionDef;
use crate::statusline::StatuslineSegmentDef;
use crate::themes::theme::ThemeDef;

pub struct RegistryDb {
	pub actions: RuntimeRegistry<ActionDef>,
	pub commands: RuntimeRegistry<CommandDef>,
	pub motions: RuntimeRegistry<MotionDef>,
	pub text_objects: crate::textobj::TextObjectRegistry,
	pub options: crate::options::OptionsRegistry,
	pub themes: RuntimeRegistry<ThemeDef>,
	pub gutters: RuntimeRegistry<GutterDef>,
	pub statusline: RuntimeRegistry<StatuslineSegmentDef>,
	pub hooks: crate::hooks::HooksRegistry,
	pub(crate) action_id_to_def: Vec<crate::core::index::DefPtr<ActionDef>>,
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
	pub key_prefixes: Vec<KeyPrefixDef>,
	#[cfg(feature = "keymap")]
	pub keymap: KeymapRegistry,
}

static DB: OnceLock<RegistryDb> = OnceLock::new();

pub fn get_db() -> &'static RegistryDb {
	DB.get_or_init(|| {
		let mut builder = builder::RegistryDbBuilder::new();

		// 1) Register builtins explicitly (not via inventory)
		if let Err(e) = builtins::register_all(&mut builder) {
			tracing::error!("Builtin registration failed: {}", e);
		}

		// 2) Run plugins (underutilized — duplicates step 1, see `run_plugins` docs)
		if let Err(e) = plugin::run_plugins(&mut builder) {
			tracing::error!("Registry plugins failed: {}", e);
		}

		let mut indices = builder.build();

		// Build numeric ID mapping for actions
		let action_id_to_def = indices.actions.items_all().to_vec();

		#[cfg(feature = "keymap")]
		let keymap = KeymapRegistry::build(&indices.actions, &indices.keybindings);

		indices.notifications.sort_by_key(|d| d.id);

		RegistryDb {
			actions: RuntimeRegistry::new("actions", indices.actions),
			commands: RuntimeRegistry::new("commands", indices.commands),
			motions: RuntimeRegistry::new("motions", indices.motions),
			text_objects: crate::textobj::TextObjectRegistry::new(indices.text_objects),
			options: crate::options::OptionsRegistry::new(indices.options),
			themes: RuntimeRegistry::new("themes", indices.themes),
			gutters: RuntimeRegistry::new("gutters", indices.gutters),
			statusline: RuntimeRegistry::new("statusline", indices.statusline),
			hooks: crate::hooks::HooksRegistry::new(indices.hooks),
			action_id_to_def,
			notifications: indices.notifications,
			key_prefixes: indices.key_prefixes,
			#[cfg(feature = "keymap")]
			keymap,
		}
	})
}

pub static ACTIONS: LazyLock<&'static RuntimeRegistry<ActionDef>> =
	LazyLock::new(|| &get_db().actions);
pub static COMMANDS: LazyLock<&'static RuntimeRegistry<CommandDef>> =
	LazyLock::new(|| &get_db().commands);
pub static MOTIONS: LazyLock<&'static RuntimeRegistry<MotionDef>> =
	LazyLock::new(|| &get_db().motions);
pub static TEXT_OBJECTS: LazyLock<&'static crate::textobj::TextObjectRegistry> =
	LazyLock::new(|| &get_db().text_objects);
pub static OPTIONS: LazyLock<&'static crate::options::OptionsRegistry> =
	LazyLock::new(|| &get_db().options);
pub static THEMES: LazyLock<&'static RuntimeRegistry<ThemeDef>> =
	LazyLock::new(|| &get_db().themes);
pub static GUTTERS: LazyLock<&'static RuntimeRegistry<GutterDef>> =
	LazyLock::new(|| &get_db().gutters);
pub static STATUSLINE_SEGMENTS: LazyLock<&'static RuntimeRegistry<StatuslineSegmentDef>> =
	LazyLock::new(|| &get_db().statusline);
pub static HOOKS: LazyLock<&'static crate::hooks::HooksRegistry> =
	LazyLock::new(|| &get_db().hooks);
pub static NOTIFICATIONS: LazyLock<&'static [&'static crate::notifications::NotificationDef]> =
	LazyLock::new(|| get_db().notifications.as_slice());

/// Resolves an ActionId to its definition.
pub fn resolve_action_id_typed(id: ActionId) -> Option<&'static ActionDef> {
	get_db()
		.action_id_to_def
		.get(id.0 as usize)
		.copied()
		.map(|p| unsafe { p.as_ref() })
}

/// Creates an ActionId from an ID string by looking it up in the registry.
pub fn resolve_action_id_from_static(id: &str) -> ActionId {
	let db = get_db();
	db.action_id_to_def
		.iter()
		.position(|&a| unsafe { a.as_ref() }.id() == id)
		.map(|pos| ActionId(pos as u32))
		.unwrap_or(ActionId::INVALID)
}
