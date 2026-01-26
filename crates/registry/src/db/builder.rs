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

	pub fn build(
		self,
	) -> (
		RegistryIndex<ActionDef>,
		RegistryIndex<CommandDef>,
		RegistryIndex<MotionDef>,
		RegistryIndex<TextObjectDef>,
		RegistryIndex<OptionDef>,
		RegistryIndex<ThemeDef>,
		RegistryIndex<GutterDef>,
		RegistryIndex<StatuslineSegmentDef>,
		RegistryIndex<HookDef>,
	) {
		(
			self.actions.build(),
			self.commands.build(),
			self.motions.build(),
			self.text_objects.build(),
			self.options.build(),
			self.themes.build(),
			self.gutters.build(),
			self.statusline.build(),
			self.hooks.build(),
		)
	}
}

impl Default for RegistryDbBuilder {
	fn default() -> Self {
		Self::new()
	}
}
