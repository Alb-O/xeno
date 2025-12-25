//! Registry builder logic - constructs indices from distributed slices.

use std::collections::HashMap;

use super::collision::{Collision, CollisionKind};
use super::diagnostics::diagnostics_internal;
use super::types::{ActionRegistryIndex, ExtensionRegistry, RegistryIndex};
use crate::{
	ACTIONS, ActionDef, ActionId, COMMANDS, CommandDef, LANGUAGES, LanguageDef, MOTIONS, MotionDef,
	RegistryMetadata, TEXT_OBJECTS, TextObjectDef,
};

/// Builds the complete extension registry from distributed slices.
///
/// Processes all registered extensions (actions, commands, motions, etc.),
/// resolves collisions based on priority, and performs validation checks.
pub(super) fn build_registry() -> ExtensionRegistry {
	let commands = build_command_index();
	let actions = build_action_index();
	let motions = build_motion_index();
	let text_objects = build_text_object_index();
	let languages = build_language_index();

	let registry = ExtensionRegistry {
		commands,
		actions,
		motions,
		text_objects,
		languages,
	};

	validate_registry(&registry);

	registry
}

fn build_command_index() -> RegistryIndex<CommandDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = COMMANDS.iter().collect();
	sorted.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for cmd in sorted {
		register_with_id_name_aliases(&mut index, cmd, cmd.id, cmd.name, cmd.aliases);
	}

	index
}

fn build_action_index() -> ActionRegistryIndex {
	let mut base = RegistryIndex::new();
	let mut by_action_id: Vec<&'static ActionDef> = Vec::new();
	let mut name_to_id: HashMap<&'static str, ActionId> = HashMap::new();
	let mut alias_to_id: HashMap<&'static str, ActionId> = HashMap::new();

	let mut sorted: Vec<_> = ACTIONS.iter().collect();
	sorted.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for action in sorted {
		if let Some(&existing) = base.by_id.get(action.id) {
			base.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: action.id.to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			base.by_id.insert(action.id, action);
		}

		if let Some(&existing) = base.by_name.get(action.name) {
			base.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: action.name.to_string(),
				winner: existing,
				shadowed: action,
			});
		} else {
			let action_id = ActionId(by_action_id.len() as u32);
			by_action_id.push(action);
			name_to_id.insert(action.name, action_id);
			base.by_name.insert(action.name, action);
		}

		for alias in action.aliases {
			if let Some(&existing) = base.by_name.get(alias) {
				base.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else if let Some(&existing) = base.by_alias.get(alias) {
				base.collisions.push(Collision {
					kind: CollisionKind::Alias,
					key: alias.to_string(),
					winner: existing,
					shadowed: action,
				});
			} else {
				base.by_alias.insert(alias, action);
				if let Some(&id) = name_to_id.get(action.name) {
					alias_to_id.insert(alias, id);
				}
			}
		}
	}

	ActionRegistryIndex {
		base,
		by_action_id,
		name_to_id,
		alias_to_id,
	}
}

fn build_motion_index() -> RegistryIndex<MotionDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = MOTIONS.iter().collect();
	sorted.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for motion in sorted {
		register_with_id_name_aliases(&mut index, motion, motion.id, motion.name, motion.aliases);
	}

	index
}

fn build_text_object_index() -> RegistryIndex<TextObjectDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = TEXT_OBJECTS.iter().collect();
	sorted.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for obj in sorted {
		register_with_id_name_aliases(&mut index, obj, obj.id, obj.name, obj.aliases);

		if let Some(&existing) = index.by_trigger.get(&obj.trigger) {
			index.collisions.push(Collision {
				kind: CollisionKind::Trigger,
				key: obj.trigger.to_string(),
				winner: existing,
				shadowed: obj,
			});
		} else {
			index.by_trigger.insert(obj.trigger, obj);
		}

		for trigger in obj.alt_triggers {
			if let Some(&existing) = index.by_trigger.get(trigger) {
				index.collisions.push(Collision {
					kind: CollisionKind::Trigger,
					key: trigger.to_string(),
					winner: existing,
					shadowed: obj,
				});
			} else {
				index.by_trigger.insert(*trigger, obj);
			}
		}
	}

	index
}

fn build_language_index() -> RegistryIndex<LanguageDef> {
	let mut index = RegistryIndex::new();
	let mut sorted: Vec<_> = LANGUAGES.iter().collect();
	sorted.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(b.id)));

	for lang in sorted {
		if let Some(&existing) = index.by_id.get(lang.id) {
			index.collisions.push(Collision {
				kind: CollisionKind::Id,
				key: lang.id.to_string(),
				winner: existing,
				shadowed: lang,
			});
		} else {
			index.by_id.insert(lang.id, lang);
		}

		if let Some(&existing) = index.by_name.get(lang.name) {
			index.collisions.push(Collision {
				kind: CollisionKind::Name,
				key: lang.name.to_string(),
				winner: existing,
				shadowed: lang,
			});
		} else {
			index.by_name.insert(lang.name, lang);
		}

		for ext in lang.extensions {
			register_alias(&mut index, lang, ext);
		}

		for fname in lang.filenames {
			register_alias(&mut index, lang, fname);
		}
	}

	index
}

/// Helper to register an item with id, name, and aliases.
fn register_with_id_name_aliases<T: RegistryMetadata>(
	index: &mut RegistryIndex<T>,
	item: &'static T,
	id: &'static str,
	name: &'static str,
	aliases: &'static [&'static str],
) {
	if let Some(&existing) = index.by_id.get(id) {
		index.collisions.push(Collision {
			kind: CollisionKind::Id,
			key: id.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_id.insert(id, item);
	}

	if let Some(&existing) = index.by_name.get(name) {
		index.collisions.push(Collision {
			kind: CollisionKind::Name,
			key: name.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_name.insert(name, item);
	}

	for alias in aliases {
		register_alias(index, item, alias);
	}
}

/// Helper to register a single alias with collision checking.
fn register_alias<T: RegistryMetadata>(
	index: &mut RegistryIndex<T>,
	item: &'static T,
	alias: &'static str,
) {
	if let Some(&existing) = index.by_name.get(alias) {
		index.collisions.push(Collision {
			kind: CollisionKind::Alias,
			key: alias.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else if let Some(&existing) = index.by_alias.get(alias) {
		index.collisions.push(Collision {
			kind: CollisionKind::Alias,
			key: alias.to_string(),
			winner: existing,
			shadowed: item,
		});
	} else {
		index.by_alias.insert(alias, item);
	}
}

/// Validates the registry and reports/panics on collisions.
fn validate_registry(reg: &ExtensionRegistry) {
	let diag = diagnostics_internal(reg);
	if diag.collisions.is_empty() {
		return;
	}

	let fatal_collisions: Vec<_> = diag
		.collisions
		.iter()
		.filter(|c| c.winner_priority == c.shadowed_priority)
		.collect();

	if !fatal_collisions.is_empty() && cfg!(debug_assertions) {
		let mut msg =
			String::from("Unresolved extension collisions (equal priority) in debug build:\n");
		for c in &fatal_collisions {
			msg.push_str(&format!(
				"  {} collision on '{}': {} (from {}) and {} (from {}) both have priority {}\n",
				c.kind,
				c.key,
				c.shadowed_id,
				c.shadowed_source,
				c.winner_id,
				c.winner_source,
				c.winner_priority
			));
		}
		msg.push_str("Please resolve these collisions by renaming or adjusting priorities.");
		panic!("{}", msg);
	}

	if cfg!(debug_assertions) {
		for c in &diag.collisions {
			if c.winner_priority != c.shadowed_priority {
				log::debug!(
					"Extension shadowing: {} '{}' from {} shadowed by {} due to priority ({} vs {})",
					c.kind,
					c.key,
					c.shadowed_source,
					c.winner_id,
					c.winner_priority,
					c.shadowed_priority
				);
			}
		}
	} else {
		log::warn!("Extension collisions detected. Use :ext doctor to resolve.");
	}
}
