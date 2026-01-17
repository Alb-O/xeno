use std::collections::HashMap;

use xeno_registry_core::{RegistryEntry, RegistrySource};

use crate::actions::ActionDef;
use crate::commands::CommandDef;
use crate::index::{ExtensionRegistry, build_registry_from_defs};
use crate::motions::MotionDef;
use crate::textobj::TextObjectDef;

#[derive(Debug)]
pub enum RegistryError {
	DuplicateId {
		kind: &'static str,
		id: &'static str,
		first: RegistrySource,
		second: RegistrySource,
	},
	DuplicateName {
		kind: &'static str,
		name: &'static str,
		first_id: &'static str,
		second_id: &'static str,
	},
	DuplicateAlias {
		kind: &'static str,
		alias: &'static str,
		first_id: &'static str,
		second_id: &'static str,
	},
}

impl std::fmt::Display for RegistryError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DuplicateId {
				kind,
				id,
				first,
				second,
			} => {
				write!(
					f,
					"duplicate {kind} id `{id}` (first: {first}, second: {second})"
				)
			}
			Self::DuplicateName {
				kind,
				name,
				first_id,
				second_id,
			} => {
				write!(
					f,
					"duplicate {kind} name `{name}` (first: {first_id}, second: {second_id})"
				)
			}
			Self::DuplicateAlias {
				kind,
				alias,
				first_id,
				second_id,
			} => {
				write!(
					f,
					"duplicate {kind} alias `{alias}` (first: {first_id}, second: {second_id})"
				)
			}
		}
	}
}

impl std::error::Error for RegistryError {}

#[derive(Debug)]
struct RegistryKindBuilder<T: RegistryEntry + 'static> {
	items: Vec<&'static T>,
	by_id: HashMap<&'static str, &'static T>,
	by_name: HashMap<&'static str, &'static T>,
	by_alias: HashMap<&'static str, &'static T>,
}

impl<T: RegistryEntry + 'static> RegistryKindBuilder<T> {
	fn new() -> Self {
		Self {
			items: Vec::new(),
			by_id: HashMap::new(),
			by_name: HashMap::new(),
			by_alias: HashMap::new(),
		}
	}

	fn register(&mut self, kind: &'static str, item: &'static T) -> Result<(), RegistryError> {
		if let Some(existing) = self.by_id.get(item.id()) {
			return Err(RegistryError::DuplicateId {
				kind,
				id: item.id(),
				first: existing.source(),
				second: item.source(),
			});
		}

		if let Some(existing) = self.by_name.get(item.name()) {
			return Err(RegistryError::DuplicateName {
				kind,
				name: item.name(),
				first_id: existing.id(),
				second_id: item.id(),
			});
		}

		self.by_id.insert(item.id(), item);
		self.by_name.insert(item.name(), item);

		for &alias in item.aliases() {
			if let Some(existing) = self.by_name.get(alias) {
				if !std::ptr::eq(*existing, item) {
					return Err(RegistryError::DuplicateAlias {
						kind,
						alias,
						first_id: existing.id(),
						second_id: item.id(),
					});
				}
				continue;
			}

			if let Some(existing) = self.by_alias.get(alias) {
				return Err(RegistryError::DuplicateAlias {
					kind,
					alias,
					first_id: existing.id(),
					second_id: item.id(),
				});
			}

			self.by_alias.insert(alias, item);
		}

		self.items.push(item);
		Ok(())
	}
}

pub struct RegistryBuilder {
	actions: RegistryKindBuilder<ActionDef>,
	commands: RegistryKindBuilder<CommandDef>,
	motions: RegistryKindBuilder<MotionDef>,
	text_objects: RegistryKindBuilder<TextObjectDef>,
}

impl RegistryBuilder {
	pub fn new() -> Self {
		Self {
			actions: RegistryKindBuilder::new(),
			commands: RegistryKindBuilder::new(),
			motions: RegistryKindBuilder::new(),
			text_objects: RegistryKindBuilder::new(),
		}
	}

	pub fn register_action(&mut self, def: &'static ActionDef) -> Result<(), RegistryError> {
		self.actions.register("action", def)
	}

	pub fn register_command(&mut self, def: &'static CommandDef) -> Result<(), RegistryError> {
		self.commands.register("command", def)
	}

	pub fn register_motion(&mut self, def: &'static MotionDef) -> Result<(), RegistryError> {
		self.motions.register("motion", def)
	}

	pub fn register_text_object(&mut self, def: &'static TextObjectDef) -> Result<(), RegistryError> {
		self.text_objects.register("text_object", def)
	}

	pub fn build(self) -> Result<ExtensionRegistry, RegistryError> {
		let RegistryBuilder {
			actions,
			commands,
			motions,
			text_objects,
		} = self;

		Ok(build_registry_from_defs(
			commands.items.as_slice(),
			actions.items.as_slice(),
			motions.items.as_slice(),
			text_objects.items.as_slice(),
		))
	}
}

impl Default for RegistryBuilder {
	fn default() -> Self {
		Self::new()
	}
}
