//! Registry database builder and per-domain accumulation helpers.

use std::collections::HashSet;
use std::sync::Arc;

use crate::core::plugin::PluginDef;
pub use crate::core::{
	ActionId, Capability, CommandError, CommandId, DuplicatePolicy, GutterId, HookId, KeyKind, LanguageId, MotionId, OptionId, RegistryBuilder, RegistryEntry,
	RegistryError, RegistryIndex, RegistryMeta, RegistrySource, RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
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
			pub(crate) plugin_ids: HashSet<&'static str>,
			pub(crate) plugin_records: Vec<PluginBuildRecord>,
		}

		pub struct RegistryIndices {
			$( $(#[$attr])* pub $field: RegistryIndex<
				<$domain as crate::db::domain::DomainSpec>::Entry,
				<$domain as crate::db::domain::DomainSpec>::Id,
			>, )*
		}

		#[derive(Debug, Clone, Copy, Default)]
		pub struct DomainCounts {
			$( $(#[$attr])* pub $field: usize, )*
		}

		impl DomainCounts {
			fn snapshot(builder: &RegistryDbBuilder) -> Self {
				Self {
					$( $(#[$attr])* $field: builder.$field.len(), )*
				}
			}

			fn diff(after: Self, before: Self) -> Self {
				Self {
					$( $(#[$attr])* $field: after.$field.saturating_sub(before.$field), )*
				}
			}
		}

		impl RegistryDbBuilder {
			pub fn new() -> Self {
				Self {
					$( $(#[$attr])* $field: RegistryBuilder::new(<$domain as crate::db::domain::DomainSpec>::LABEL), )*
					plugin_ids: HashSet::new(),
					plugin_records: Vec::new(),
				}
			}

			pub fn build(self) -> RegistryIndices {
				RegistryIndices {
					$( $(#[$attr])* $field: self.$field.build(), )*
				}
			}
		}
	}
}

define_domains! {
	{ stem: action, domain: crate::db::domains::Actions, field: actions }
	{ stem: command, domain: crate::db::domains::Commands, field: commands }
	{ stem: motion, domain: crate::db::domains::Motions, field: motions }
	{ stem: text_object, domain: crate::db::domains::TextObjects, field: text_objects }
	{ stem: option, domain: crate::db::domains::Options, field: options }
	#[cfg(feature = "commands")]
	{ stem: snippet, domain: crate::db::domains::Snippets, field: snippets }
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
	pub fn push_domain<D: crate::db::domain::DomainSpec>(&mut self, input: D::Input) {
		D::on_push(self, &input);
		D::builder(self).push(Arc::new(input));
	}

	pub fn plugin_build_records(&self) -> &[PluginBuildRecord] {
		&self.plugin_records
	}

	pub fn register_plugin(&mut self, plugin: &'static PluginDef) -> Result<(), RegistryError> {
		if !self.plugin_ids.insert(plugin.meta.id) {
			return Err(RegistryError::Plugin(format!("duplicate plugin id {}", plugin.meta.id)));
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
			"OptionDef default type mismatch: name={} key={} value_type={:?} default_type={:?}",
			def.meta.name,
			def.key,
			def.value_type,
			def.default.value_type(),
		);
	}
}
