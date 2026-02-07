use std::collections::HashSet;

use crate::actions::definition::ActionEntry;
use crate::actions::{ActionDef, KeyBindingDef, KeyPrefixDef};
use crate::commands::{CommandDef, CommandEntry};
use crate::core::plugin::PluginDef;
pub use crate::core::{
	ActionId, Capability, CommandError, CommandId, DuplicatePolicy, GutterId, HookId, KeyKind,
	MotionId, OptionId, RegistryBuilder, RegistryEntry, RegistryError, RegistryIndex, RegistryMeta,
	RegistrySource, RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
};
use crate::gutter::{GutterDef, GutterEntry};
use crate::hooks::{HookDef, HookEntry};
use crate::motions::{MotionDef, MotionEntry};
use crate::options::{OptionDef, OptionEntry};
use crate::statusline::{StatuslineEntry, StatuslineSegmentDef};
use crate::textobj::{TextObjectDef, TextObjectEntry};
use crate::themes::theme::{ThemeDef, ThemeEntry};

#[derive(Debug)]
pub struct BuiltinGroup<T: 'static> {
	pub name: &'static str,
	pub defs: &'static [&'static T],
}

impl<T> BuiltinGroup<T> {
	pub const fn new(name: &'static str, defs: &'static [&'static T]) -> Self {
		Self { name, defs }
	}
}

macro_rules! impl_group_reg {
	($fn_name:ident, $ty:ty, $item_fn:ident, $domain:literal) => {
		pub fn $fn_name(&mut self, group: &'static BuiltinGroup<$ty>) {
			let span = tracing::debug_span!(
				"builtin.group",
				domain = $domain,
				group = group.name,
				count = group.defs.len(),
			);
			let _guard = span.enter();
			for &def in group.defs {
				self.$item_fn(def);
			}
		}
	};
}

pub struct RegistryDbBuilder {
	pub actions: RegistryBuilder<ActionDef, ActionEntry, ActionId>,
	pub commands: RegistryBuilder<CommandDef, CommandEntry, CommandId>,
	pub motions: RegistryBuilder<MotionDef, MotionEntry, MotionId>,
	pub text_objects: RegistryBuilder<TextObjectDef, TextObjectEntry, TextObjectId>,
	pub options: RegistryBuilder<OptionDef, OptionEntry, OptionId>,
	pub themes: RegistryBuilder<ThemeDef, ThemeEntry, ThemeId>,
	pub gutters: RegistryBuilder<GutterDef, GutterEntry, GutterId>,
	pub statusline: RegistryBuilder<StatuslineSegmentDef, StatuslineEntry, StatuslineId>,
	pub hooks: RegistryBuilder<HookDef, HookEntry, HookId>,
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
		self.actions.push_static(def);
		self.keybindings.extend(def.bindings.iter().copied());
	}

	pub fn register_command(&mut self, def: &'static CommandDef) {
		self.commands.push_static(def);
	}

	pub fn register_motion(&mut self, def: &'static MotionDef) {
		self.motions.push_static(def);
	}

	pub fn register_text_object(&mut self, def: &'static TextObjectDef) {
		self.text_objects.push_static(def);
	}

	pub fn register_option(&mut self, def: &'static OptionDef) {
		validate_option_def(def);
		self.options.push_static(def);
	}

	pub fn register_theme(&mut self, def: &'static ThemeDef) {
		self.themes.push_static(def);
	}

	pub fn register_gutter(&mut self, def: &'static GutterDef) {
		self.gutters.push_static(def);
	}

	pub fn register_statusline_segment(&mut self, def: &'static StatuslineSegmentDef) {
		self.statusline.push_static(def);
	}

	pub fn register_hook(&mut self, def: &'static HookDef) {
		self.hooks.push_static(def);
	}

	pub fn register_notification(&mut self, def: &'static crate::notifications::NotificationDef) {
		self.notifications.push(def);
	}

	pub fn register_key_prefixes(&mut self, defs: &'static [KeyPrefixDef]) {
		self.key_prefixes.extend(defs.iter().copied());
	}

	impl_group_reg!(register_action_group, ActionDef, register_action, "actions");
	impl_group_reg!(
		register_command_group,
		CommandDef,
		register_command,
		"commands"
	);
	impl_group_reg!(register_motion_group, MotionDef, register_motion, "motions");
	impl_group_reg!(
		register_text_object_group,
		TextObjectDef,
		register_text_object,
		"text_objects"
	);
	impl_group_reg!(register_option_group, OptionDef, register_option, "options");
	impl_group_reg!(register_theme_group, ThemeDef, register_theme, "themes");
	impl_group_reg!(register_gutter_group, GutterDef, register_gutter, "gutters");
	impl_group_reg!(
		register_statusline_group,
		StatuslineSegmentDef,
		register_statusline_segment,
		"statusline"
	);
	impl_group_reg!(register_hook_group, HookDef, register_hook, "hooks");
	impl_group_reg!(
		register_notification_group,
		crate::notifications::NotificationDef,
		register_notification,
		"notifications"
	);

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
		let _diff = DomainCounts::diff(after, before);

		self.plugin_records.push(PluginBuildRecord {
			plugin_id: plugin.meta.id,
			counts: _diff,
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
