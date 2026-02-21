//! Registry catalog construction and global accessor surfaces.
//!
//! Domain wiring is generated from `domains::catalog`, so builder fields, runtime
//! fields, and global accessors stay in sync.

use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock, OnceLock};

pub use crate::core::{
	ActionId, CommandId, DenseId, GutterId, HookId, LanguageId, MotionId, OptionId, RegistryIndex, RuntimeRegistry, SnippetId, StatuslineId, TextObjectId,
	ThemeId,
};

pub mod builder;
pub mod builtins;
pub mod domain;
pub mod index;
#[cfg(feature = "keymap")]
pub mod keymap_registry;

use crate::actions::entry::ActionEntry;
#[cfg(feature = "keymap")]
use crate::db::keymap_registry::KeymapSnapshotCache;
use crate::domains::catalog::with_registry_domains;

/// Marker trait for typed domain access over [`RegistryCatalog`].
pub trait CatalogDomain: crate::db::domain::DomainSpec {
	fn domain(catalog: &RegistryCatalog) -> &Self::Runtime;
}

macro_rules! define_registry_catalog {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				global: $global:ident,
				marker: $marker:path,
				$(,)?
			}
		)*
	) => {
		pub struct RegistryCatalog {
			$( $(#[$attr])* pub $field: <$marker as crate::db::domain::DomainSpec>::Runtime, )*
			#[cfg(feature = "keymap")]
			pub keymap: KeymapSnapshotCache,
			version_hash: u64,
		}

		$(
			$(#[$attr])*
			impl CatalogDomain for $marker {
				fn domain(catalog: &RegistryCatalog) -> &Self::Runtime {
					&catalog.$field
				}
			}
		)*
	};
}

with_registry_domains!(define_registry_catalog);

#[derive(Debug, thiserror::Error)]
pub enum CatalogLoadError {
	#[error("builtin registration failed: {0}")]
	BuiltinRegistration(#[from] crate::core::RegistryError),
	#[error("invalid cross-domain references: {0:?}")]
	InvalidCrossDomainReferences(Vec<String>),
}

impl RegistryCatalog {
	/// Loads the immutable registry catalog from compiled builtins.
	pub fn load() -> Result<Self, CatalogLoadError> {
		let mut builder = builder::RegistryDbBuilder::new();
		builtins::register_all(&mut builder)?;
		Self::from_indices(builder.build())
	}

	fn from_indices(indices: builder::RegistryIndices) -> Result<Self, CatalogLoadError> {
		let actions_reg = <crate::actions::Actions as crate::db::domain::DomainSpec>::into_runtime(indices.actions);

		#[cfg(feature = "keymap")]
		let keymap = KeymapSnapshotCache::new(0, actions_reg.snapshot());

		macro_rules! domain_runtime {
			(actions, $marker:path, $indices:ident, $actions_reg:ident) => {
				$actions_reg
			};
			($field:ident, $marker:path, $indices:ident, $actions_reg:ident) => {
				<$marker as crate::db::domain::DomainSpec>::into_runtime($indices.$field)
			};
		}

		macro_rules! init_registry_catalog {
			(
				$(
					$(#[$attr:meta])*
					{
						field: $field:ident,
						global: $global:ident,
						marker: $marker:path,
						$(,)?
					}
				)*
			) => {
				RegistryCatalog {
					$( $(#[$attr])* $field: domain_runtime!($field, $marker, indices, actions_reg), )*
					#[cfg(feature = "keymap")]
					keymap,
					version_hash: 0,
				}
			};
		}

		let mut catalog = with_registry_domains!(init_registry_catalog);
		catalog.validate_cross_domain_references()?;
		catalog.version_hash = hash_catalog(&catalog);
		#[cfg(feature = "keymap")]
		{
			catalog.keymap = KeymapSnapshotCache::new(catalog.version_hash, catalog.actions.snapshot());
		}
		Ok(catalog)
	}

	/// Returns a typed runtime view for domain marker `D`.
	pub fn domain<D>(&self) -> &D::Runtime
	where
		D: CatalogDomain,
	{
		D::domain(self)
	}

	/// Stable hash of the loaded immutable catalog contents.
	pub fn version_hash(&self) -> u64 {
		self.version_hash
	}

	/// Collects diagnostics across all domains in the catalog.
	pub fn diagnostics(&self) -> CatalogDiagnostics {
		CatalogDiagnostics {
			collisions: collect_catalog_collisions(self),
		}
	}

	pub fn notifications_reg(&self) -> &RuntimeRegistry<crate::notifications::NotificationEntry, crate::notifications::NotificationId> {
		&self.notifications
	}

	fn validate_cross_domain_references(&self) -> Result<(), CatalogLoadError> {
		crate::domains::relations::language_lsp::validate_language_lsp_references(&self.languages, &self.lsp_servers)
			.map_err(CatalogLoadError::InvalidCrossDomainReferences)
	}
}

macro_rules! define_catalog_hash_fn {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				global: $global:ident,
				marker: $marker:path,
				$(,)?
			}
		)*
	) => {
		fn hash_catalog(catalog: &RegistryCatalog) -> u64 {
			let mut hasher = rustc_hash::FxHasher::default();
			$(
				$(#[$attr])*
				for entry in catalog.$field.snapshot_guard().iter_refs() {
					entry.id_str().hash(&mut hasher);
					entry.name_str().hash(&mut hasher);
				}
			)*
			hasher.finish()
		}
	};
}

with_registry_domains!(define_catalog_hash_fn);

/// Catalog-wide diagnostics artifact.
pub struct CatalogDiagnostics {
	pub collisions: Vec<crate::core::Collision>,
}

macro_rules! define_catalog_collision_fn {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				global: $global:ident,
				marker: $marker:path,
				$(,)?
			}
		)*
	) => {
		fn collect_catalog_collisions(catalog: &RegistryCatalog) -> Vec<crate::core::Collision> {
			let mut collisions = Vec::new();
			$(
				$(#[$attr])*
				collisions.extend(catalog.$field.collisions().iter().cloned());
			)*
			collisions.sort_by(|a, b| a.stable_cmp(b));
			collisions
		}
	};
}

with_registry_domains!(define_catalog_collision_fn);

macro_rules! define_registry_globals {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				global: $global:ident,
				marker: $marker:path,
				$(,)?
			}
		)*
	) => {
		$( $(#[$attr])* pub static $global: LazyLock<&'static <$marker as crate::db::domain::DomainSpec>::Runtime> = LazyLock::new(|| &get_catalog().$field); )*
	};
}

static CATALOG_CELL: OnceLock<RegistryCatalog> = OnceLock::new();

/// Global immutable registry catalog.
pub static CATALOG: LazyLock<&'static RegistryCatalog> = LazyLock::new(get_catalog);

pub fn get_catalog() -> &'static RegistryCatalog {
	CATALOG_CELL.get_or_init(|| RegistryCatalog::load().unwrap_or_else(|error| panic!("failed to load registry catalog: {error}")))
}

with_registry_domains!(define_registry_globals);

pub fn resolve_action_id_typed(id: ActionId) -> Option<Arc<ActionEntry>> {
	ACTIONS.get_by_id(id).map(|r| r.get_arc())
}

pub fn resolve_action_id_from_static(id: &str) -> ActionId {
	let db = get_catalog();
	db.actions.get(id).map(|r: crate::actions::ActionRef| r.dense_id()).unwrap_or(ActionId::INVALID)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::core::{RegistryMetaStatic, RegistrySource};
	use crate::languages::types::LanguageDef;
	use crate::languages::{LanguageInput, Languages};

	static INVALID_LANGUAGE_DEF: LanguageDef = LanguageDef {
		meta: RegistryMetaStatic {
			id: "test::language::invalid_lsp_ref",
			name: "invalid_lsp_ref",
			keys: &[],
			description: "test language with unknown lsp server",
			priority: 0,
			source: RegistrySource::Builtin,
			mutates_buffer: false,
		},
		scope: None,
		grammar_name: None,
		injection_regex: None,
		auto_format: false,
		extensions: &["xeno-invalid"],
		filenames: &[],
		globs: &[],
		shebangs: &[],
		comment_tokens: &[],
		block_comment: None,
		lsp_servers: &["missing-server"],
		roots: &[],
	};

	#[test]
	fn catalog_load_is_deterministic() {
		let first = RegistryCatalog::load().expect("catalog load should succeed");
		let second = RegistryCatalog::load().expect("catalog load should succeed");
		assert_eq!(first.version_hash(), second.version_hash());
		assert_eq!(first.actions.len(), second.actions.len());
		assert_eq!(first.commands.len(), second.commands.len());
	}

	#[test]
	fn unknown_lsp_server_reference_fails_catalog_build() {
		let mut builder = builder::RegistryDbBuilder::new();
		builder.push_domain::<Languages>(LanguageInput::Static(INVALID_LANGUAGE_DEF.clone()));

		let error = match RegistryCatalog::from_indices(builder.build()) {
			Ok(_) => panic!("catalog should reject unknown lsp server references"),
			Err(error) => error,
		};
		match error {
			CatalogLoadError::InvalidCrossDomainReferences(missing) => {
				assert_eq!(missing.len(), 1);
				assert!(missing[0].contains("missing-server"), "error should include referenced missing server");
			}
			other => panic!("unexpected error variant: {other:?}"),
		}
	}
}
