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
use crate::themes::theme::{ThemeDef, ThemeEntry, ThemeInput};

pub struct RegistryDbBuilder {
	pub actions: RegistryBuilder<ActionInput, ActionEntry, ActionId>,
	pub commands: RegistryBuilder<CommandInput, CommandEntry, CommandId>,
	pub motions: RegistryBuilder<MotionInput, MotionEntry, MotionId>,
	pub text_objects: RegistryBuilder<TextObjectInput, TextObjectEntry, TextObjectId>,
	pub options: RegistryBuilder<OptionInput, OptionEntry, OptionId>,
	pub themes: RegistryBuilder<ThemeInput, ThemeEntry, ThemeId>,
	pub gutters: RegistryBuilder<GutterInput, GutterEntry, GutterId>,
	pub statusline: RegistryBuilder<StatuslineInput, StatuslineEntry, StatuslineId>,
	pub hooks: RegistryBuilder<HookInput, HookEntry, HookId>,
	pub notifications: RegistryBuilder<
		crate::notifications::NotificationInput,
		crate::notifications::NotificationEntry,
		crate::notifications::NotificationId,
	>,
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
	pub notifications: RegistryIndex<
		crate::notifications::NotificationEntry,
		crate::notifications::NotificationId,
	>,
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
			notifications: RegistryBuilder::new("notifications"),
			keybindings: Vec::new(),
			key_prefixes: Vec::new(),
			plugin_ids: HashSet::new(),
			plugin_records: Vec::new(),
		}
	}

	fn push_domain<D: crate::db::domain::DomainSpec>(&mut self, input: D::Input) {
		D::on_push(self, &input);
		D::builder(self).push(Arc::new(input));
	}

	pub fn register_action(&mut self, def: &'static ActionDef) {
		self.push_domain::<crate::db::domains::Actions>(ActionInput::Static(def.clone()));
	}

	/// Registers an action defined via KDL metadata + Rust handler linking.
	pub fn register_linked_action(&mut self, def: LinkedActionDef) {
		self.push_domain::<crate::db::domains::Actions>(ActionInput::Linked(def));
	}

	pub fn register_command(&mut self, def: &'static CommandDef) {
		self.push_domain::<crate::db::domains::Commands>(CommandInput::Static(def.clone()));
	}

	/// Registers a command defined via KDL metadata + Rust handler linking.
	pub fn register_linked_command(&mut self, def: crate::kdl::link::LinkedCommandDef) {
		self.push_domain::<crate::db::domains::Commands>(CommandInput::Linked(def));
	}

	pub fn register_motion(&mut self, def: &'static MotionDef) {
		self.push_domain::<crate::db::domains::Motions>(MotionInput::Static(def.clone()));
	}

	/// Registers a motion defined via KDL metadata + Rust handler linking.
	pub fn register_linked_motion(&mut self, def: crate::kdl::link::LinkedMotionDef) {
		self.push_domain::<crate::db::domains::Motions>(MotionInput::Linked(def));
	}

	pub fn register_text_object(&mut self, def: &'static TextObjectDef) {
		self.push_domain::<crate::db::domains::TextObjects>(TextObjectInput::Static(*def));
	}

	/// Registers a text object defined via KDL metadata + Rust handler linking.
	pub fn register_linked_text_object(&mut self, def: crate::kdl::link::LinkedTextObjectDef) {
		self.push_domain::<crate::db::domains::TextObjects>(TextObjectInput::Linked(def));
	}

	pub fn register_option(&mut self, def: &'static OptionDef) {
		self.push_domain::<crate::db::domains::Options>(OptionInput::Static(def.clone()));
	}

	/// Registers an option defined via KDL metadata + Rust validator linking.
	pub fn register_linked_option(&mut self, def: crate::options::def::LinkedOptionDef) {
		self.push_domain::<crate::db::domains::Options>(OptionInput::Linked(def));
	}

	pub fn register_theme(&mut self, def: &'static ThemeDef) {
		self.push_domain::<crate::db::domains::Themes>(ThemeInput::Static(*def));
	}

	/// Registers a theme defined via KDL metadata.
	pub fn register_linked_theme(&mut self, def: crate::themes::theme::LinkedThemeDef) {
		self.push_domain::<crate::db::domains::Themes>(ThemeInput::Linked(def));
	}

	pub fn register_gutter(&mut self, def: &'static GutterDef) {
		self.push_domain::<crate::db::domains::Gutters>(GutterInput::Static(*def));
	}

	/// Registers a gutter defined via KDL metadata + Rust handler linking.
	pub fn register_linked_gutter(&mut self, def: crate::kdl::link::LinkedGutterDef) {
		self.push_domain::<crate::db::domains::Gutters>(GutterInput::Linked(def));
	}

	pub fn register_statusline_segment(&mut self, def: &'static StatuslineSegmentDef) {
		self.push_domain::<crate::db::domains::Statusline>(StatuslineInput::Static(*def));
	}

	/// Registers a statusline segment defined via KDL metadata + Rust handler linking.
	pub fn register_linked_statusline_segment(
		&mut self,
		def: crate::kdl::link::LinkedStatuslineDef,
	) {
		self.push_domain::<crate::db::domains::Statusline>(StatuslineInput::Linked(def));
	}

	pub fn register_hook(&mut self, def: &'static HookDef) {
		self.push_domain::<crate::db::domains::Hooks>(HookInput::Static(*def));
	}

	/// Registers a hook defined via KDL metadata + Rust handler linking.
	pub fn register_linked_hook(&mut self, def: crate::kdl::link::LinkedHookDef) {
		self.push_domain::<crate::db::domains::Hooks>(HookInput::Linked(def));
	}

	pub fn register_notification(&mut self, def: &'static crate::notifications::NotificationDef) {
		self.push_domain::<crate::db::domains::Notifications>(
			crate::notifications::NotificationInput::Static(*def),
		);
	}

	/// Registers a notification defined via KDL metadata.
	pub fn register_linked_notification(
		&mut self,
		def: crate::notifications::def::LinkedNotificationDef,
	) {
		self.push_domain::<crate::db::domains::Notifications>(
			crate::notifications::NotificationInput::Linked(def),
		);
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
			notifications: self.notifications.build(),
			keybindings: self.keybindings,
			key_prefixes: self.key_prefixes,
		}
	}
}

/// Validates that an option definition's default value matches its declared type.
pub(crate) fn validate_option_def(def: &OptionDef) {
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
