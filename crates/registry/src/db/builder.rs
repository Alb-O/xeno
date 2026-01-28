use crate::actions::ActionDef;
use crate::commands::CommandDef;
pub use crate::core::{
	Capability, CommandError, DuplicatePolicy, KeyKind, RegistryBuilder, RegistryEntry,
	RegistryIndex, RegistryMeta, RegistrySource, RuntimeRegistry, insert_typed_key,
};
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
	Insert(#[from] crate::core::InsertFatal),
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
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
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
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
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
			notifications: Vec::new(),
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

	pub fn register_option(&mut self, def: &'static OptionDef) {
		self.options.push(def);
	}

	pub fn register_theme(&mut self, def: &'static ThemeDef) {
		self.themes.push(def);
	}

	pub fn register_gutter(&mut self, def: &'static GutterDef) {
		self.gutters.push(def);
	}

	pub fn register_statusline_segment(&mut self, def: &'static StatuslineSegmentDef) {
		self.statusline.push(def);
	}

	pub fn register_hook(&mut self, def: &'static HookDef) {
		self.hooks.push(def);
	}

	pub fn register_notification(&mut self, def: &'static crate::notifications::NotificationDef) {
		self.notifications.push(def);
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
			notifications: self.notifications,
		}
	}
}

impl Default for RegistryDbBuilder {
	fn default() -> Self {
		Self::new()
	}
}
