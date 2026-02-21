//! Registry database builder and per-domain accumulation helpers.

use std::sync::Arc;

pub use crate::core::{
	ActionId, CommandError, CommandId, DuplicatePolicy, GutterId, HookId, KeyKind, LanguageId, MotionId, OptionId, RegistryBuilder, RegistryEntry, RegistryError,
	RegistryIndex, RegistryMeta, RegistrySource, RuntimeRegistry, StatuslineId, TextObjectId, ThemeId,
};
use crate::options::OptionDef;

macro_rules! define_domains {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				global: $global:ident,
				marker: $domain:path,
				$(,)?
			}
		)*
	) => {
		pub struct RegistryDbBuilder {
			$( $(#[$attr])* pub $field: RegistryBuilder<
				<$domain as crate::db::domain::DomainSpec>::Input,
				<$domain as crate::db::domain::DomainSpec>::Entry,
				<$domain as crate::db::domain::DomainSpec>::Id,
			>, )*
		}

		pub struct RegistryIndices {
			$( $(#[$attr])* pub $field: RegistryIndex<
				<$domain as crate::db::domain::DomainSpec>::Entry,
				<$domain as crate::db::domain::DomainSpec>::Id,
			>, )*
		}

		impl RegistryDbBuilder {
			pub fn new() -> Self {
				Self {
					$( $(#[$attr])* $field: RegistryBuilder::new(<$domain as crate::db::domain::DomainSpec>::LABEL), )*
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

crate::domains::catalog::with_registry_domains!(define_domains);

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
