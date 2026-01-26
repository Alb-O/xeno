pub use xeno_registry_core::{
	Capability, CommandError, DuplicatePolicy, KeyKind, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistrySource, RuntimeRegistry, insert_typed_key,
};

use crate::actions::ActionDef;
use crate::commands::CommandDef;
use crate::gutter::GutterDef;
use crate::hooks::HookDef;
use crate::motions::MotionDef;
use crate::options::OptionDef;
use crate::statusline::StatuslineSegmentDef;
use crate::textobj::TextObjectDef;
use crate::themes::theme::ThemeDef;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
	#[error("fatal insertion error: {0}")]
	Insert(#[from] xeno_registry_core::InsertFatal),
	#[error("plugin error: {0}")]
	Plugin(String),
}

pub struct RegistryDbBuilder {
	pub actions: RegistryBuilder<ActionDef>,
	pub commands: RegistryBuilder<CommandDef>,
	pub motions: RegistryBuilder<MotionDef>,
	pub text_objects: RegistryBuilder<TextObjectDef>,
	pub options: RegistryBuilder<OptionDef>,
	pub themes: RegistryBuilder<ThemeDef>,
	pub gutters: RegistryBuilder<GutterDef>,
	pub statusline: RegistryBuilder<StatuslineSegmentDef>,
	pub hooks: RegistryBuilder<HookDef>,
}

pub struct RegistryIndices {
	pub actions: RegistryIndex<ActionDef>,
	pub commands: RegistryIndex<CommandDef>,
	pub motions: RegistryIndex<MotionDef>,
	pub text_objects: RegistryIndex<TextObjectDef>,
	pub options: RegistryIndex<OptionDef>,
	pub themes: RegistryIndex<ThemeDef>,
	pub gutters: RegistryIndex<GutterDef>,
	pub statusline: RegistryIndex<StatuslineSegmentDef>,
	pub hooks: RegistryIndex<HookDef>,
}

impl RegistryDbBuilder {
	pub fn new() -> Self {
		Self {
			actions: RegistryBuilder::new("actions"),
			commands: RegistryBuilder::new("commands"),
			motions: RegistryBuilder::new("motions"),
			text_objects: RegistryBuilder::new("text_objects"),
			options: RegistryBuilder::new("options"),
			themes: RegistryBuilder::new("themes"),
			gutters: RegistryBuilder::new("gutters"),
			statusline: RegistryBuilder::new("statusline"),
			hooks: RegistryBuilder::new("hooks"),
		}
	}

	pub fn register_action(&mut self, def: &'static ActionDef) {
		self.actions.push(def);
	}

	pub fn register_command(&mut self, def: &'static CommandDef) {
		self.commands.push(def);
	}

	pub fn register_motion(&mut self, def: &'static MotionDef) {
		self.motions.push(def);
	}

	pub fn register_text_object(&mut self, def: &'static TextObjectDef) {
		self.text_objects.push(def);
	}

	pub fn build(self) -> RegistryIndices {
		RegistryIndices {
			actions: self.actions.build(),
			commands: self.commands.build(),
			motions: self.motions.build(),
			text_objects: self.text_objects.build(),
			options: self.options.build(),
			themes: self.themes.build(),
			gutters: self.gutters.build(),
			statusline: self.statusline.build(),
			hooks: self.hooks.build(),
		}
	}
}

impl Default for RegistryDbBuilder {
	fn default() -> Self {
		Self::new()
	}
}
