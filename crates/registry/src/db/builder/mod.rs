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

macro_rules! define_domains {
	(
		$(
			$(#[$attr:meta])*
			$domain_id:ident : {
				stem: $stem:ident,
				domain: $domain:path,
				field: $field:ident,
				input: $input:ty,
				entry: $entry:ty,
				id: $id_ty:ty,
				static_def: $static_def:ty,
				static_to_input: $static_to_input:expr,
				linked_def: $linked_def:ty,
				linked_to_input: $linked_to_input:expr $(,)?
			}
		)*
	) => {
		pub struct RegistryDbBuilder {
			$( $(#[$attr])* pub $field: RegistryBuilder<$input, $entry, $id_ty>, )*
			pub keybindings: Vec<KeyBindingDef>,
			pub key_prefixes: Vec<KeyPrefixDef>,
			pub(crate) plugin_ids: HashSet<&'static str>,
			pub(crate) plugin_records: Vec<PluginBuildRecord>,
		}

		pub struct RegistryIndices {
			$( $(#[$attr])* pub $field: RegistryIndex<$entry, $id_ty>, )*
			pub keybindings: Vec<KeyBindingDef>,
			pub key_prefixes: Vec<KeyPrefixDef>,
		}

		#[derive(Debug, Clone, Copy, Default)]
		pub struct DomainCounts {
			$( $(#[$attr])* pub $field: usize, )*
			pub keybindings: usize,
			pub key_prefixes: usize,
		}

		impl DomainCounts {
			fn snapshot(builder: &RegistryDbBuilder) -> Self {
				Self {
					$( $(#[$attr])* $field: builder.$field.len(), )*
					keybindings: builder.keybindings.len(),
					key_prefixes: builder.key_prefixes.len(),
				}
			}

			fn diff(after: Self, before: Self) -> Self {
				Self {
					$( $(#[$attr])* $field: after.$field.saturating_sub(before.$field), )*
					keybindings: after.keybindings.saturating_sub(before.keybindings),
					key_prefixes: after.key_prefixes.saturating_sub(before.key_prefixes),
				}
			}
		}

		impl RegistryDbBuilder {
			pub fn new() -> Self {
				Self {
					$( $(#[$attr])* $field: RegistryBuilder::new(stringify!($field)), )*
					keybindings: Vec::new(),
					key_prefixes: Vec::new(),
					plugin_ids: HashSet::new(),
					plugin_records: Vec::new(),
				}
			}

			pub fn build(self) -> RegistryIndices {
				RegistryIndices {
					$( $(#[$attr])* $field: self.$field.build(), )*
					keybindings: self.keybindings,
					key_prefixes: self.key_prefixes,
				}
			}

			$(
				paste::paste! {
					$(#[$attr])*
					pub fn [<register_ $stem>](&mut self, def: &'static $static_def) {
						let func = $static_to_input;
						self.push_domain::<$domain>(func(def));
					}

					$(#[$attr])*
					pub fn [<register_linked_ $stem>](&mut self, def: $linked_def) {
						let func = $linked_to_input;
						self.push_domain::<$domain>(func(def));
					}
				}
			)*
		}
	}
}

define_domains! {
	actions: {
		stem: action,
		domain: crate::db::domains::Actions,
		field: actions,
		input: ActionInput,
		entry: ActionEntry,
		id: ActionId,
		static_def: ActionDef,
		static_to_input: |def: &'static ActionDef| ActionInput::Static(def.clone()),
		linked_def: LinkedActionDef,
		linked_to_input: |def: LinkedActionDef| ActionInput::Linked(def),
	}
	commands: {
		stem: command,
		domain: crate::db::domains::Commands,
		field: commands,
		input: CommandInput,
		entry: CommandEntry,
		id: CommandId,
		static_def: CommandDef,
		static_to_input: |def: &'static CommandDef| CommandInput::Static(def.clone()),
		linked_def: crate::kdl::link::LinkedCommandDef,
		linked_to_input: |def: crate::kdl::link::LinkedCommandDef| CommandInput::Linked(def),
	}
	motions: {
		stem: motion,
		domain: crate::db::domains::Motions,
		field: motions,
		input: MotionInput,
		entry: MotionEntry,
		id: MotionId,
		static_def: MotionDef,
		static_to_input: |def: &'static MotionDef| MotionInput::Static(def.clone()),
		linked_def: crate::kdl::link::LinkedMotionDef,
		linked_to_input: |def: crate::kdl::link::LinkedMotionDef| MotionInput::Linked(def),
	}
	text_objects: {
		stem: text_object,
		domain: crate::db::domains::TextObjects,
		field: text_objects,
		input: TextObjectInput,
		entry: TextObjectEntry,
		id: TextObjectId,
		static_def: TextObjectDef,
		static_to_input: |def: &'static TextObjectDef| TextObjectInput::Static(*def),
		linked_def: crate::kdl::link::LinkedTextObjectDef,
		linked_to_input: |def: crate::kdl::link::LinkedTextObjectDef| TextObjectInput::Linked(def),
	}
	options: {
		stem: option,
		domain: crate::db::domains::Options,
		field: options,
		input: OptionInput,
		entry: OptionEntry,
		id: OptionId,
		static_def: OptionDef,
		static_to_input: |def: &'static OptionDef| OptionInput::Static(def.clone()),
		linked_def: crate::options::def::LinkedOptionDef,
		linked_to_input: |def: crate::options::def::LinkedOptionDef| OptionInput::Linked(def),
	}
	themes: {
		stem: theme,
		domain: crate::db::domains::Themes,
		field: themes,
		input: ThemeInput,
		entry: ThemeEntry,
		id: ThemeId,
		static_def: ThemeDef,
		static_to_input: |def: &'static ThemeDef| ThemeInput::Static(*def),
		linked_def: crate::themes::theme::LinkedThemeDef,
		linked_to_input: |def: crate::themes::theme::LinkedThemeDef| ThemeInput::Linked(def),
	}
	gutters: {
		stem: gutter,
		domain: crate::db::domains::Gutters,
		field: gutters,
		input: GutterInput,
		entry: GutterEntry,
		id: GutterId,
		static_def: GutterDef,
		static_to_input: |def: &'static GutterDef| GutterInput::Static(*def),
		linked_def: crate::kdl::link::LinkedGutterDef,
		linked_to_input: |def: crate::kdl::link::LinkedGutterDef| GutterInput::Linked(def),
	}
	statusline: {
		stem: statusline_segment,
		domain: crate::db::domains::Statusline,
		field: statusline,
		input: StatuslineInput,
		entry: StatuslineEntry,
		id: StatuslineId,
		static_def: StatuslineSegmentDef,
		static_to_input: |def: &'static StatuslineSegmentDef| StatuslineInput::Static(*def),
		linked_def: crate::kdl::link::LinkedStatuslineDef,
		linked_to_input: |def: crate::kdl::link::LinkedStatuslineDef| StatuslineInput::Linked(def),
	}
	hooks: {
		stem: hook,
		domain: crate::db::domains::Hooks,
		field: hooks,
		input: HookInput,
		entry: HookEntry,
		id: HookId,
		static_def: HookDef,
		static_to_input: |def: &'static HookDef| HookInput::Static(*def),
		linked_def: crate::kdl::link::LinkedHookDef,
		linked_to_input: |def: crate::kdl::link::LinkedHookDef| HookInput::Linked(def),
	}
	notifications: {
		stem: notification,
		domain: crate::db::domains::Notifications,
		field: notifications,
		input: crate::notifications::NotificationInput,
		entry: crate::notifications::NotificationEntry,
		id: crate::notifications::NotificationId,
		static_def: crate::notifications::NotificationDef,
		static_to_input: |def: &'static crate::notifications::NotificationDef| crate::notifications::NotificationInput::Static(*def),
		linked_def: crate::notifications::def::LinkedNotificationDef,
		linked_to_input: |def: crate::notifications::def::LinkedNotificationDef| crate::notifications::NotificationInput::Linked(def),
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
	pub fn push_domain<D: crate::db::domain::DomainSpec>(&mut self, input: D::Input) {
		D::on_push(self, &input);
		D::builder(self).push(Arc::new(input));
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
