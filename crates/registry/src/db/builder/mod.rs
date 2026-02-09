use std::collections::HashSet;
use std::sync::Arc;

use crate::actions::{KeyBindingDef, KeyPrefixDef};
use crate::core::plugin::PluginDef;
pub use crate::core::{
	ActionId, Capability, CommandError, CommandId, DuplicatePolicy, GutterId, HookId, KeyKind,
	LanguageId, MotionId, OptionId, RegistryBuilder, RegistryEntry, RegistryError, RegistryIndex,
	RegistryMeta, RegistrySource, RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
};
use crate::options::OptionDef;

macro_rules! define_domains {
	(
		$(
			$(#[$attr:meta])*
			{
				stem: $stem:ident,
				domain: $domain:path,
				field: $field:ident $(,)?
			}
		)*
	) => {
		pub struct RegistryDbBuilder {
			$( $(#[$attr])* pub $field: RegistryBuilder<
				<$domain as crate::db::domain::DomainSpec>::Input,
				<$domain as crate::db::domain::DomainSpec>::Entry,
				<$domain as crate::db::domain::DomainSpec>::Id,
			>, )*
			pub keybindings: Vec<KeyBindingDef>,
			pub key_prefixes: Vec<KeyPrefixDef>,
			pub(crate) plugin_ids: HashSet<&'static str>,
			pub(crate) plugin_records: Vec<PluginBuildRecord>,
		}

		pub struct RegistryIndices {
			$( $(#[$attr])* pub $field: RegistryIndex<
				<$domain as crate::db::domain::DomainSpec>::Entry,
				<$domain as crate::db::domain::DomainSpec>::Id,
			>, )*
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
					$( $(#[$attr])* $field: RegistryBuilder::new(<$domain as crate::db::domain::DomainSpec>::LABEL), )*
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
					pub fn [<register_ $stem>](&mut self, def: &'static <$domain as crate::db::domain::DomainSpec>::StaticDef) {
						self.push_domain::<$domain>(<$domain as crate::db::domain::DomainSpec>::static_to_input(def));
					}

					$(#[$attr])*
					pub fn [<register_linked_ $stem>](&mut self, def: <$domain as crate::db::domain::DomainSpec>::LinkedDef) {
						self.push_domain::<$domain>(<$domain as crate::db::domain::DomainSpec>::linked_to_input(def));
					}
				}
			)*
		}
	}
}

define_domains! {
	{ stem: action, domain: crate::db::domains::Actions, field: actions }
	{ stem: command, domain: crate::db::domains::Commands, field: commands }
	{ stem: motion, domain: crate::db::domains::Motions, field: motions }
	{ stem: text_object, domain: crate::db::domains::TextObjects, field: text_objects }
	{ stem: option, domain: crate::db::domains::Options, field: options }
	{ stem: theme, domain: crate::db::domains::Themes, field: themes }
	{ stem: gutter, domain: crate::db::domains::Gutters, field: gutters }
	{ stem: statusline_segment, domain: crate::db::domains::Statusline, field: statusline }
	{ stem: hook, domain: crate::db::domains::Hooks, field: hooks }
	{ stem: notification, domain: crate::db::domains::Notifications, field: notifications }
	{ stem: language, domain: crate::db::domains::Languages, field: languages }
	{ stem: lsp_server, domain: crate::db::domains::LspServers, field: lsp_servers }
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
	#[cfg(feature = "actions")]
	pub fn register_compiled_actions(&mut self) {
		let spec = crate::actions::loader::load_actions_spec();
		let handlers = inventory::iter::<crate::actions::handler::ActionHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::actions::link::link_actions(&spec, handlers);

		for def in linked {
			self.register_linked_action(def);
		}

		self.register_key_prefixes(crate::actions::link::link_prefixes(&spec));
	}

	#[cfg(feature = "commands")]
	pub fn register_compiled_commands(&mut self) {
		let spec = crate::commands::loader::load_commands_spec();
		let handlers = inventory::iter::<crate::commands::CommandHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::commands::link::link_commands(&spec, handlers);

		for def in linked {
			self.register_linked_command(def);
		}
	}

	pub fn register_compiled_motions(&mut self) {
		let spec = crate::motions::loader::load_motions_spec();
		let handlers = inventory::iter::<crate::motions::handler::MotionHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::motions::link::link_motions(&spec, handlers);

		for def in linked {
			self.register_linked_motion(def);
		}
	}

	pub fn register_compiled_text_objects(&mut self) {
		let spec = crate::textobj::loader::load_text_objects_spec();
		let handlers = inventory::iter::<crate::textobj::handler::TextObjectHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::textobj::link::link_text_objects(&spec, handlers);

		for def in linked {
			self.register_linked_text_object(def);
		}
	}

	pub fn register_compiled_options(&mut self) {
		let spec = crate::options::loader::load_options_spec();
		let validators = inventory::iter::<crate::options::OptionValidatorReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::options::link::link_options(&spec, validators);

		for def in linked {
			self.register_linked_option(def);
		}
	}

	pub fn register_compiled_hooks(&mut self) {
		let spec = crate::hooks::loader::load_hooks_spec();
		let handlers = inventory::iter::<crate::hooks::handler::HookHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::hooks::link::link_hooks(&spec, handlers);

		for def in linked {
			self.register_linked_hook(def);
		}
	}

	pub fn register_compiled_statusline(&mut self) {
		let spec = crate::statusline::loader::load_statusline_spec();
		let handlers = inventory::iter::<crate::statusline::handler::StatuslineHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::statusline::link::link_statusline(&spec, handlers);

		for def in linked {
			self.register_linked_statusline_segment(def);
		}
	}

	pub fn register_compiled_gutters(&mut self) {
		let spec = crate::gutter::loader::load_gutters_spec();
		let handlers = inventory::iter::<crate::gutter::handler::GutterHandlerReg>
			.into_iter()
			.map(|r| r.0);

		let linked = crate::gutter::link::link_gutters(&spec, handlers);

		for def in linked {
			self.register_linked_gutter(def);
		}
	}

	pub fn register_compiled_notifications(&mut self) {
		let spec = crate::notifications::loader::load_notifications_spec();
		let linked = crate::notifications::link::link_notifications(&spec);

		for def in linked {
			self.register_linked_notification(def);
		}
	}

	pub fn register_compiled_themes(&mut self) {
		let spec = crate::themes::loader::load_themes_spec();
		let linked = crate::themes::link::link_themes(&spec);

		for def in linked {
			self.register_linked_theme(def);
		}
	}

	pub fn register_compiled_languages(&mut self) {
		let spec = crate::languages::loader::load_languages_spec();
		let linked = crate::languages::link::link_languages(&spec);

		for def in linked {
			self.register_linked_language(def);
		}
	}

	pub fn register_compiled_lsp_servers(&mut self) {
		let spec = crate::lsp_servers::loader::load_lsp_servers_spec();
		let linked = crate::lsp_servers::entry::link_lsp_servers(&spec);

		for def in linked {
			self.register_linked_lsp_server(def);
		}
	}

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
