//! Registry database construction and global accessor surfaces.

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
use crate::commands::CommandEntry;
use crate::db::domain_catalog::with_registry_domains;
#[cfg(feature = "keymap")]
use crate::db::keymap_registry::KeymapSnapshotCache;
use crate::gutter::GutterEntry;
use crate::hooks::registry::HooksRegistry;
use crate::languages::LanguagesRegistry;
use crate::lsp_servers::LspServersRegistry;
use crate::motions::MotionEntry;
use crate::options::registry::OptionsRegistry;
#[cfg(feature = "commands")]
use crate::snippets::SnippetEntry;
use crate::statusline::StatuslineEntry;
use crate::textobj::registry::TextObjectRegistry;
use crate::themes::theme::ThemeEntry;

macro_rules! define_registry_db {
	(
		$(
			$(#[$attr:meta])*
			{
				field: $field:ident,
				marker: $marker:path,
				runtime_ty: $runtime_ty:ty,
				init: $init:expr $(,)?
			}
		)*
	) => {
		pub struct RegistryDb {
			$( $(#[$attr])* pub $field: $runtime_ty, )*
			#[cfg(feature = "keymap")]
			pub keymap: KeymapSnapshotCache,
		}
	};
}

with_registry_domains!(define_registry_db);

static DB: OnceLock<RegistryDb> = OnceLock::new();

pub fn get_db() -> &'static RegistryDb {
	DB.get_or_init(|| {
		let mut builder = builder::RegistryDbBuilder::new();

		if let Err(e) = builtins::register_all(&mut builder) {
			tracing::error!("Builtin registration failed: {}", e);
		}

		let indices = builder.build();

		let actions_reg = RuntimeRegistry::new("actions", indices.actions);

		#[cfg(feature = "keymap")]
		let keymap = KeymapSnapshotCache::new(actions_reg.snapshot());

		macro_rules! init_registry_db {
			(
				$(
					$(#[$attr:meta])*
					{
						field: $field:ident,
						marker: $marker:path,
						runtime_ty: $runtime_ty:ty,
						init: $init:expr $(,)?
					}
				)*
			) => {
				RegistryDb {
					$( $(#[$attr])* $field: $init, )*
					#[cfg(feature = "keymap")]
					keymap,
				}
			};
		}

		with_registry_domains!(init_registry_db, indices, actions_reg)
	})
}

pub static ACTIONS: LazyLock<&'static RuntimeRegistry<ActionEntry, ActionId>> = LazyLock::new(|| &get_db().actions);
pub static COMMANDS: LazyLock<&'static RuntimeRegistry<CommandEntry, CommandId>> = LazyLock::new(|| &get_db().commands);
pub static MOTIONS: LazyLock<&'static RuntimeRegistry<MotionEntry, MotionId>> = LazyLock::new(|| &get_db().motions);
pub static TEXT_OBJECTS: LazyLock<&'static TextObjectRegistry> = LazyLock::new(|| &get_db().text_objects);
pub static OPTIONS: LazyLock<&'static OptionsRegistry> = LazyLock::new(|| &get_db().options);
#[cfg(feature = "commands")]
pub static SNIPPETS: LazyLock<&'static RuntimeRegistry<SnippetEntry, SnippetId>> = LazyLock::new(|| &get_db().snippets);
pub static THEMES: LazyLock<&'static RuntimeRegistry<ThemeEntry, ThemeId>> = LazyLock::new(|| &get_db().themes);
pub static GUTTERS: LazyLock<&'static RuntimeRegistry<GutterEntry, GutterId>> = LazyLock::new(|| &get_db().gutters);
pub static STATUSLINE_SEGMENTS: LazyLock<&'static RuntimeRegistry<StatuslineEntry, StatuslineId>> = LazyLock::new(|| &get_db().statusline);
pub static HOOKS: LazyLock<&'static HooksRegistry> = LazyLock::new(|| &get_db().hooks);
pub static NOTIFICATIONS: LazyLock<&'static RuntimeRegistry<crate::notifications::NotificationEntry, crate::notifications::NotificationId>> =
	LazyLock::new(|| &get_db().notifications);
pub static LANGUAGES: LazyLock<&'static LanguagesRegistry> = LazyLock::new(|| &get_db().languages);
pub static LSP_SERVERS: LazyLock<&'static LspServersRegistry> = LazyLock::new(|| &get_db().lsp_servers);

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
