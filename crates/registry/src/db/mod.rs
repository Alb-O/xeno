//! Registry database construction and global accessor surfaces.
//!
//! Domain wiring is generated from `domain_catalog`, so builder fields, runtime
//! fields, and global accessors stay in sync.

use std::sync::{Arc, LazyLock, OnceLock};

pub use crate::core::{
	ActionId, CommandId, DenseId, GutterId, HookId, LanguageId, MotionId, OptionId, RegistryIndex, RuntimeRegistry, SnippetId, StatuslineId, TextObjectId,
	ThemeId,
};

pub mod builder;
pub mod builtins;
pub mod domain;
mod domain_catalog;
pub mod index;
#[cfg(feature = "keymap")]
pub mod keymap_registry;

use crate::actions::entry::ActionEntry;
use crate::db::domain_catalog::with_registry_domains;
#[cfg(feature = "keymap")]
use crate::db::keymap_registry::KeymapSnapshotCache;

macro_rules! define_registry_db {
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
		pub struct RegistryDb {
			$( $(#[$attr])* pub $field: <$marker as crate::db::domain::DomainSpec>::Runtime, )*
			#[cfg(feature = "keymap")]
			pub keymap: KeymapSnapshotCache,
		}
	};
}

with_registry_domains!(define_registry_db);

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
		$( $(#[$attr])* pub static $global: LazyLock<&'static <$marker as crate::db::domain::DomainSpec>::Runtime> = LazyLock::new(|| &get_db().$field); )*
	};
}

struct RegistryBootstrap;

impl RegistryBootstrap {
	fn build() -> RegistryDb {
		let mut builder = builder::RegistryDbBuilder::new();

		if let Err(e) = builtins::register_all(&mut builder) {
			tracing::error!("Builtin registration failed: {}", e);
		}

		Self::from_indices(builder.build())
	}

	fn from_indices(indices: builder::RegistryIndices) -> RegistryDb {
		let actions_reg = <crate::actions::Actions as crate::db::domain::DomainSpec>::into_runtime(indices.actions);

		#[cfg(feature = "keymap")]
		let keymap = KeymapSnapshotCache::new(actions_reg.snapshot());

		macro_rules! domain_runtime {
			(actions, $marker:path, $indices:ident, $actions_reg:ident) => {
				$actions_reg
			};
			($field:ident, $marker:path, $indices:ident, $actions_reg:ident) => {
				<$marker as crate::db::domain::DomainSpec>::into_runtime($indices.$field)
			};
		}

		macro_rules! init_registry_db {
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
				RegistryDb {
					$( $(#[$attr])* $field: domain_runtime!($field, $marker, indices, actions_reg), )*
					#[cfg(feature = "keymap")]
					keymap,
				}
			};
		}

		with_registry_domains!(init_registry_db)
	}
}

static DB: OnceLock<RegistryDb> = OnceLock::new();

pub fn get_db() -> &'static RegistryDb {
	DB.get_or_init(RegistryBootstrap::build)
}

with_registry_domains!(define_registry_globals);

impl RegistryDb {
	pub fn notifications_reg(&self) -> &RuntimeRegistry<crate::notifications::NotificationEntry, crate::notifications::NotificationId> {
		&self.notifications
	}
}

pub fn resolve_action_id_typed(id: ActionId) -> Option<Arc<ActionEntry>> {
	ACTIONS.get_by_id(id).map(|r| r.get_arc())
}

pub fn resolve_action_id_from_static(id: &str) -> ActionId {
	let db = get_db();
	db.actions.get(id).map(|r: crate::actions::ActionRef| r.dense_id()).unwrap_or(ActionId::INVALID)
}
