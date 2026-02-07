use std::collections::HashSet;
use std::sync::Arc;

use crate::actions::def::ActionInput;
use crate::actions::entry::ActionEntry;
use crate::actions::{ActionDef, KeyBindingDef, KeyPrefixDef};
use crate::commands::def::CommandInput;
use crate::commands::{CommandDef, CommandEntry};
use crate::core::plugin::PluginDef;
pub use crate::core::{
	ActionId, Capability, CommandError, CommandId, DuplicatePolicy, GutterId, HookId, KeyKind,
	MotionId, OptionId, RegistryBuilder, RegistryEntry, RegistryError, RegistryIndex, RegistryMeta,
	RegistrySource, RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
};
use crate::gutter::{GutterDef, GutterEntry, GutterInput};
use crate::hooks::{HookDef, HookEntry, HookInput};
#[cfg(feature = "actions")]
use crate::kdl::link::LinkedActionDef;
use crate::motions::{MotionDef, MotionEntry, MotionInput};
use crate::options::{OptionDef, OptionEntry, OptionInput};
use crate::statusline::{StatuslineEntry, StatuslineInput, StatuslineSegmentDef};
use crate::textobj::{TextObjectDef, TextObjectEntry, TextObjectInput};
use crate::themes::theme::{ThemeDef, ThemeEntry};

pub struct RegistryDbBuilder {
	pub actions: RegistryBuilder<ActionInput, ActionEntry, ActionId>,
	pub commands: RegistryBuilder<CommandInput, CommandEntry, CommandId>,
	pub motions: RegistryBuilder<MotionInput, MotionEntry, MotionId>,
	pub text_objects: RegistryBuilder<TextObjectInput, TextObjectEntry, TextObjectId>,
	pub options: RegistryBuilder<OptionInput, OptionEntry, OptionId>,
	pub themes: RegistryBuilder<ThemeDef, ThemeEntry, ThemeId>,
	pub gutters: RegistryBuilder<GutterInput, GutterEntry, GutterId>,
	pub statusline: RegistryBuilder<StatuslineInput, StatuslineEntry, StatuslineId>,
	pub hooks: RegistryBuilder<HookInput, HookEntry, HookId>,
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
	pub keybindings: Vec<KeyBindingDef>,
	pub key_prefixes: Vec<KeyPrefixDef>,
	plugin_ids: HashSet<&'static str>,
	plugin_records: Vec<PluginBuildRecord>,
}

pub struct RegistryIndices {
	pub actions: RegistryIndex<ActionEntry, ActionId>,
	pub commands: RegistryIndex<CommandEntry, CommandId>,
	pub motions: RegistryIndex<MotionEntry, MotionId>,
	pub text_objects: RegistryIndex<TextObjectEntry, TextObjectId>,
	pub options: RegistryIndex<OptionEntry, OptionId>,
	pub themes: RegistryIndex<ThemeEntry, ThemeId>,
	pub gutters: RegistryIndex<GutterEntry, GutterId>,
	pub statusline: RegistryIndex<StatuslineEntry, StatuslineId>,
	pub hooks: RegistryIndex<HookEntry, HookId>,
	pub notifications: Vec<&'static crate::notifications::NotificationDef>,
	pub keybindings: Vec<KeyBindingDef>,
	pub key_prefixes: Vec<KeyPrefixDef>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DomainCounts {
	pub actions: usize,
	pub commands: usize,
	pub motions: usize,
	pub text_objects: usize,
	pub options: usize,
	pub themes: usize,
	pub gutters: usize,
	pub statusline: usize,
	pub hooks: usize,
	pub notifications: usize,
	pub keybindings: usize,
	pub key_prefixes: usize,
}

impl DomainCounts {
	fn snapshot(builder: &RegistryDbBuilder) -> Self {
		Self {
			actions: builder.actions.len(),
			commands: builder.commands.len(),
			motions: builder.motions.len(),
			text_objects: builder.text_objects.len(),
			options: builder.options.len(),
			themes: builder.themes.len(),
			gutters: builder.gutters.len(),
			statusline: builder.statusline.len(),
			hooks: builder.hooks.len(),
			notifications: builder.notifications.len(),
			keybindings: builder.keybindings.len(),
			key_prefixes: builder.key_prefixes.len(),
		}
	}

	fn diff(after: Self, before: Self) -> Self {
		Self {
			actions: after.actions.saturating_sub(before.actions),
			commands: after.commands.saturating_sub(before.commands),
			motions: after.motions.saturating_sub(before.motions),
			text_objects: after.text_objects.saturating_sub(before.text_objects),
			options: after.options.saturating_sub(before.options),
			themes: after.themes.saturating_sub(before.themes),
			gutters: after.gutters.saturating_sub(before.gutters),
			statusline: after.statusline.saturating_sub(before.statusline),
			hooks: after.hooks.saturating_sub(before.hooks),
			notifications: after.notifications.saturating_sub(before.notifications),
			keybindings: after.keybindings.saturating_sub(before.keybindings),
			key_prefixes: after.key_prefixes.saturating_sub(before.key_prefixes),
		}
	}
}

#[derive(Debug)]
pub struct PluginBuildRecord {
	pub plugin_id: &'static str,
	pub counts: DomainCounts,
}

fn validate_option_def(def: &'static OptionDef) {
	if def.default.value_type() != def.value_type {
		panic!(
			"OptionDef default type mismatch: name={} kdl_key={} value_type={:?} default_type={:?}",
			def.meta.name,
			def.kdl_key,
			def.value_type,
			def.default.value_type(),
		);
	}
}

impl Default for RegistryDbBuilder {
	fn default() -> Self {
		Self::new()
	}
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
			keybindings: Vec::new(),
			key_prefixes: Vec::new(),
			plugin_ids: HashSet::new(),
			plugin_records: Vec::new(),
		}
	}

	pub fn register_action(&mut self, def: &'static ActionDef) {
		self.keybindings.extend(def.bindings.iter().cloned());
		self.actions
			.push(Arc::new(ActionInput::Static(def.clone())));
	}

	/// Registers an action defined via KDL metadata + Rust handler linking.
	pub fn register_linked_action(&mut self, def: LinkedActionDef) {
		self.keybindings.extend(def.bindings.iter().cloned());
		self.actions.push(Arc::new(ActionInput::Linked(def)));
	}

	pub fn register_command(&mut self, def: &'static CommandDef) {
		self.commands
			.push(Arc::new(CommandInput::Static(def.clone())));
	}

	/// Registers a command defined via KDL metadata + Rust handler linking.
	pub fn register_linked_command(&mut self, def: crate::kdl::link::LinkedCommandDef) {
		self.commands.push(Arc::new(CommandInput::Linked(def)));
	}

	pub fn register_motion(&mut self, def: &'static MotionDef) {
		self.motions
			.push(Arc::new(MotionInput::Static(def.clone())));
	}

	/// Registers a motion defined via KDL metadata + Rust handler linking.
	pub fn register_linked_motion(&mut self, def: crate::kdl::link::LinkedMotionDef) {
		self.motions.push(Arc::new(MotionInput::Linked(def)));
	}

	pub fn register_text_object(&mut self, def: &'static TextObjectDef) {
		self.text_objects
			.push(Arc::new(TextObjectInput::Static(*def)));
	}

	/// Registers a text object defined via KDL metadata + Rust handler linking.
	pub fn register_linked_text_object(&mut self, def: crate::kdl::link::LinkedTextObjectDef) {
		self.text_objects
			.push(Arc::new(TextObjectInput::Linked(def)));
	}

	pub fn register_option(&mut self, def: &'static OptionDef) {
		validate_option_def(def);
		self.options.push(Arc::new(OptionInput::Static(*def)));
	}

	pub fn register_theme(&mut self, def: &'static ThemeDef) {
		self.themes.push_static(def);
	}

	pub fn register_gutter(&mut self, def: &'static GutterDef) {
		self.gutters.push(Arc::new(GutterInput::Static(*def)));
	}

	/// Registers a gutter defined via KDL metadata + Rust handler linking.
	pub fn register_linked_gutter(&mut self, def: crate::kdl::link::LinkedGutterDef) {
		self.gutters.push(Arc::new(GutterInput::Linked(def)));
	}

	pub fn register_statusline_segment(&mut self, def: &'static StatuslineSegmentDef) {
		self.statusline
			.push(Arc::new(StatuslineInput::Static(*def)));
	}

	/// Registers a statusline segment defined via KDL metadata + Rust handler linking.
	pub fn register_linked_statusline_segment(
		&mut self,
		def: crate::kdl::link::LinkedStatuslineDef,
	) {
		self.statusline.push(Arc::new(StatuslineInput::Linked(def)));
	}

	pub fn register_hook(&mut self, def: &'static HookDef) {
		self.hooks.push(Arc::new(HookInput::Static(*def)));
	}

	/// Registers a hook defined via KDL metadata + Rust handler linking.
	pub fn register_linked_hook(&mut self, def: crate::kdl::link::LinkedHookDef) {
		self.hooks.push(Arc::new(HookInput::Linked(def)));
	}

	pub fn register_notification(&mut self, def: &'static crate::notifications::NotificationDef) {
		self.notifications.push(def);
	}

	pub fn register_key_prefixes(&mut self, defs: impl IntoIterator<Item = KeyPrefixDef>) {
		self.key_prefixes.extend(defs);
	}

	pub fn plugin_build_records(&self) -> &[PluginBuildRecord] {
		&self.plugin_records
	}

	pub fn register_plugin(&mut self, plugin: &'static PluginDef) -> Result<(), RegistryError> {
		if !self.plugin_ids.insert(plugin.meta.id) {
			return Err(RegistryError::Plugin(format!(
				"duplicate plugin id {}",
				plugin.meta.id
			)));
		}

		let before = DomainCounts::snapshot(self);
		(plugin.register)(self)?;
		let after = DomainCounts::snapshot(self);
		let diff = DomainCounts::diff(after, before);

		self.plugin_records.push(PluginBuildRecord {
			plugin_id: plugin.meta.id,
			counts: diff,
		});

		Ok(())
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
			keybindings: self.keybindings,
			key_prefixes: self.key_prefixes,
		}
	}
}
