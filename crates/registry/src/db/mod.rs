use std::sync::{LazyLock, OnceLock};

pub use xeno_registry_core::{ActionId, RegistryIndex, RuntimeRegistry};

pub mod builder;
pub mod index;
#[cfg(feature = "keymap")]
pub mod keymap_registry;
pub mod plugin;

use inventory;

use crate::actions::ActionDef;
use crate::commands::CommandDef;
use crate::gutter::GutterDef;
use crate::hooks::HookDef;
use crate::motions::MotionDef;
use crate::options::OptionDef;
use crate::statusline::StatuslineSegmentDef;
use crate::textobj::TextObjectDef;
use crate::themes::theme::ThemeDef;

pub struct RegistryDb {
	pub actions: RuntimeRegistry<ActionDef>,
	pub commands: RuntimeRegistry<CommandDef>,
	pub motions: RuntimeRegistry<MotionDef>,
	pub text_objects: RuntimeRegistry<TextObjectDef>,
	pub options: RuntimeRegistry<OptionDef>,
	pub themes: RuntimeRegistry<ThemeDef>,
	pub gutters: RuntimeRegistry<GutterDef>,
	pub statusline: RuntimeRegistry<StatuslineSegmentDef>,
	pub hooks: RuntimeRegistry<HookDef>,
	pub(crate) action_id_to_def: Vec<&'static ActionDef>,
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
}

static DB: OnceLock<RegistryDb> = OnceLock::new();

pub fn get_db() -> &'static RegistryDb {
	DB.get_or_init(|| {
		let mut builder = builder::RegistryDbBuilder::new();

		// Collect from inventory
		builder.actions.extend(
			inventory::iter::<crate::inventory::Reg<ActionDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.commands.extend(
			inventory::iter::<crate::inventory::Reg<CommandDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.motions.extend(
			inventory::iter::<crate::inventory::Reg<MotionDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.text_objects.extend(
			inventory::iter::<crate::inventory::Reg<TextObjectDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.options.extend(
			inventory::iter::<crate::options::OptionReg>
				.into_iter()
				.map(|r| r.0),
		);
		builder.themes.extend(
			inventory::iter::<crate::inventory::Reg<ThemeDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.gutters.extend(
			inventory::iter::<crate::inventory::Reg<GutterDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.statusline.extend(
			inventory::iter::<crate::inventory::Reg<StatuslineSegmentDef>>
				.into_iter()
				.map(|r| r.0),
		);
		builder.hooks.extend(
			inventory::iter::<crate::inventory::Reg<HookDef>>
				.into_iter()
				.map(|r| r.0),
		);

		// Run plugins
		if let Err(e) = plugin::run_plugins(&mut builder) {
			tracing::error!("Registry plugins failed: {}", e);
		}

		let (actions, commands, motions, text_objects, options, themes, gutters, statusline, hooks) =
			builder.build();

		// Build numeric ID mapping for actions
		let action_id_to_def = actions.items_all().to_vec();

		let mut notifications: Vec<_> =
			inventory::iter::<crate::inventory::Reg<crate::notifications::NotificationDef>>
				.into_iter()
				.map(|r| r.0)
				.collect();
		notifications.sort_by_key(|d| d.id);

		RegistryDb {
			actions: RuntimeRegistry::new("actions", actions),
			commands: RuntimeRegistry::new("commands", commands),
			motions: RuntimeRegistry::new("motions", motions),
			text_objects: RuntimeRegistry::new("text_objects", text_objects),
			options: RuntimeRegistry::new("options", options),
			themes: RuntimeRegistry::new("themes", themes),
			gutters: RuntimeRegistry::new("gutters", gutters),
			statusline: RuntimeRegistry::new("statusline", statusline),
			hooks: RuntimeRegistry::new("hooks", hooks),
			action_id_to_def,
			notifications,
		}
	})
}

pub static ACTIONS: LazyLock<&'static RuntimeRegistry<ActionDef>> =
	LazyLock::new(|| &get_db().actions);
pub static COMMANDS: LazyLock<&'static RuntimeRegistry<CommandDef>> =
	LazyLock::new(|| &get_db().commands);
pub static MOTIONS: LazyLock<&'static RuntimeRegistry<MotionDef>> =
	LazyLock::new(|| &get_db().motions);
pub static TEXT_OBJECTS: LazyLock<&'static RuntimeRegistry<TextObjectDef>> =
	LazyLock::new(|| &get_db().text_objects);
pub static OPTIONS: LazyLock<&'static RuntimeRegistry<OptionDef>> =
	LazyLock::new(|| &get_db().options);
pub static THEMES: LazyLock<&'static RuntimeRegistry<ThemeDef>> =
	LazyLock::new(|| &get_db().themes);
pub static GUTTERS: LazyLock<&'static RuntimeRegistry<GutterDef>> =
	LazyLock::new(|| &get_db().gutters);
pub static STATUSLINE_SEGMENTS: LazyLock<&'static RuntimeRegistry<StatuslineSegmentDef>> =
	LazyLock::new(|| &get_db().statusline);
pub static HOOKS: LazyLock<&'static RuntimeRegistry<HookDef>> = LazyLock::new(|| &get_db().hooks);
pub static NOTIFICATIONS: LazyLock<&'static [&'static crate::notifications::NotificationDef]> =
	LazyLock::new(|| get_db().notifications.as_slice());

pub static TEXT_OBJECT_TRIGGER_INDEX: LazyLock<
	std::collections::HashMap<char, &'static TextObjectDef>,
> = LazyLock::new(|| {
	let mut map = std::collections::HashMap::new();
	for def in TEXT_OBJECTS.iter() {
		map.entry(def.trigger).or_insert(def);
		for &alt in def.alt_triggers {
			map.entry(alt).or_insert(def);
		}
	}
	map
});

pub static OPTION_KDL_INDEX: LazyLock<std::collections::HashMap<&'static str, &'static OptionDef>> =
	LazyLock::new(|| {
		let mut map = std::collections::HashMap::new();
		for opt in OPTIONS.iter() {
			map.insert(opt.kdl_key, opt);
		}
		map
	});

pub static BUILTIN_HOOK_BY_EVENT: LazyLock<
	std::collections::HashMap<crate::hooks::HookEvent, Vec<&'static HookDef>>,
> = LazyLock::new(|| {
	let mut map: std::collections::HashMap<crate::hooks::HookEvent, Vec<&'static HookDef>> =
		std::collections::HashMap::new();
	for hook in HOOKS.builtins().iter() {
		map.entry(hook.event).or_default().push(hook);
	}
	// Sort each event's hooks by priority (asc), then name (asc), then id (asc)
	for hooks in map.values_mut() {
		hooks.sort_by(|a, b| {
			a.meta
				.priority
				.cmp(&b.meta.priority)
				.then_with(|| a.meta.name.cmp(b.meta.name))
				.then_with(|| a.meta.id.cmp(b.meta.id))
		});
	}
	map
});

/// Resolves an ActionId to its definition.
pub fn resolve_action_id_typed(id: ActionId) -> Option<&'static ActionDef> {
	get_db().action_id_to_def.get(id.0 as usize).copied()
}

/// Creates an ActionId from a static ID string by looking it up in the registry.
pub fn resolve_action_id_from_static(id: &'static str) -> ActionId {
	let db = get_db();
	db.action_id_to_def
		.iter()
		.position(|&a| a.id() == id)
		.map(|pos| ActionId(pos as u32))
		.unwrap_or(ActionId::INVALID)
}
