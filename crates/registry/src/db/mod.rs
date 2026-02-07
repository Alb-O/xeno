use std::sync::{Arc, LazyLock, OnceLock};

pub use crate::core::{
	ActionId, CommandId, DenseId, GutterId, HookId, MotionId, OptionId, RegistryIndex,
	RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
};

pub mod builder;
pub mod builtins;
pub mod index;
#[cfg(feature = "keymap")]
pub mod keymap_registry;
pub mod plugin;

use crate::actions::KeyPrefixDef;
use crate::actions::definition::ActionEntry;
use crate::commands::CommandEntry;
#[cfg(feature = "keymap")]
use crate::db::keymap_registry::KeymapRegistry;
use crate::gutter::GutterEntry;
use crate::hooks::registry::HooksRegistry;
use crate::motions::MotionEntry;
use crate::options::registry::OptionsRegistry;
use crate::statusline::StatuslineEntry;
use crate::textobj::registry::TextObjectRegistry;
use crate::themes::theme::ThemeEntry;

pub struct RegistryDb {
	pub actions: RuntimeRegistry<ActionEntry, ActionId>,
	pub commands: RuntimeRegistry<CommandEntry, CommandId>,
	pub motions: RuntimeRegistry<MotionEntry, MotionId>,
	pub text_objects: TextObjectRegistry,
	pub options: OptionsRegistry,
	pub themes: RuntimeRegistry<ThemeEntry, ThemeId>,
	pub gutters: RuntimeRegistry<GutterEntry, GutterId>,
	pub statusline: RuntimeRegistry<StatuslineEntry, StatuslineId>,
	pub hooks: HooksRegistry,
	pub(crate) action_id_to_def: Vec<Arc<ActionEntry>>,
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
	pub key_prefixes: Vec<KeyPrefixDef>,
	#[cfg(feature = "keymap")]
	pub keymap: KeymapRegistry,
}

static DB: OnceLock<RegistryDb> = OnceLock::new();

pub fn get_db() -> &'static RegistryDb {
	DB.get_or_init(|| {
		let mut builder = builder::RegistryDbBuilder::new();

		if let Err(e) = builtins::register_all(&mut builder) {
			tracing::error!("Builtin registration failed: {}", e);
		}

		if let Err(e) = plugin::run_plugins(&mut builder) {
			tracing::error!("Registry plugins failed: {}", e);
		}

		let indices = builder.build();

		let action_id_to_def = indices.actions.items().to_vec();

		#[cfg(feature = "keymap")]
		let keymap = KeymapRegistry::build(&indices.actions, &indices.keybindings);

		let mut notifications = indices.notifications;
		notifications.sort_by_key(|d| d.id);

		RegistryDb {
			actions: RuntimeRegistry::new("actions", indices.actions),
			commands: RuntimeRegistry::new("commands", indices.commands),
			motions: RuntimeRegistry::new("motions", indices.motions),
			text_objects: TextObjectRegistry::new(indices.text_objects),
			options: OptionsRegistry::new(indices.options),
			themes: RuntimeRegistry::new("themes", indices.themes),
			gutters: RuntimeRegistry::new("gutters", indices.gutters),
			statusline: RuntimeRegistry::new("statusline", indices.statusline),
			hooks: HooksRegistry::new(indices.hooks),
			action_id_to_def,
			notifications,
			key_prefixes: indices.key_prefixes,
			#[cfg(feature = "keymap")]
			keymap,
		}
	})
}

pub static ACTIONS: LazyLock<&'static RuntimeRegistry<ActionEntry, ActionId>> =
	LazyLock::new(|| &get_db().actions);
pub static COMMANDS: LazyLock<&'static RuntimeRegistry<CommandEntry, CommandId>> =
	LazyLock::new(|| &get_db().commands);
pub static MOTIONS: LazyLock<&'static RuntimeRegistry<MotionEntry, MotionId>> =
	LazyLock::new(|| &get_db().motions);
pub static TEXT_OBJECTS: LazyLock<&'static TextObjectRegistry> =
	LazyLock::new(|| &get_db().text_objects);
pub static OPTIONS: LazyLock<&'static OptionsRegistry> = LazyLock::new(|| &get_db().options);
pub static THEMES: LazyLock<&'static RuntimeRegistry<ThemeEntry, ThemeId>> =
	LazyLock::new(|| &get_db().themes);
pub static GUTTERS: LazyLock<&'static RuntimeRegistry<GutterEntry, GutterId>> =
	LazyLock::new(|| &get_db().gutters);
pub static STATUSLINE_SEGMENTS: LazyLock<&'static RuntimeRegistry<StatuslineEntry, StatuslineId>> =
	LazyLock::new(|| &get_db().statusline);
pub static HOOKS: LazyLock<&'static HooksRegistry> = LazyLock::new(|| &get_db().hooks);
pub static NOTIFICATIONS: LazyLock<&'static [&'static crate::notifications::NotificationDef]> =
	LazyLock::new(|| get_db().notifications.as_slice());

pub fn resolve_action_id_typed(id: ActionId) -> Option<Arc<ActionEntry>> {
	get_db().action_id_to_def.get(id.0 as usize).cloned()
}

pub fn resolve_action_id_from_static(id: &str) -> ActionId {
	let db = get_db();
	db.actions
		.get(id)
		.map(|r| r.dense_id())
		.unwrap_or(ActionId::INVALID)
}
