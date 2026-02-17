//! Shared domain wiring catalog used by both builder and runtime DB assembly.
//!
//! This is the single source of truth for domain field names, marker types,
//! runtime container types, global accessor names, and runtime construction
//! expressions.

macro_rules! with_registry_domains {
	($callback:ident) => {
		$crate::db::domain_catalog::with_registry_domains!($callback, __registry_indices, __actions_reg);
	};
	($callback:ident, $indices:ident, $actions_reg:ident) => {
		$callback! {
			{
				field: actions,
				global: ACTIONS,
				marker: crate::actions::Actions,
				runtime_ty: crate::core::RuntimeRegistry<crate::actions::entry::ActionEntry, crate::core::ActionId>,
				init: $actions_reg,
			}
			{
				field: commands,
				global: COMMANDS,
				marker: crate::commands::Commands,
				runtime_ty: crate::core::RuntimeRegistry<crate::commands::entry::CommandEntry, crate::core::CommandId>,
				init: crate::core::RuntimeRegistry::new("commands", $indices.commands),
			}
			{
				field: motions,
				global: MOTIONS,
				marker: crate::motions::Motions,
				runtime_ty: crate::core::RuntimeRegistry<crate::motions::MotionEntry, crate::core::MotionId>,
				init: crate::core::RuntimeRegistry::new("motions", $indices.motions),
			}
			{
				field: text_objects,
				global: TEXT_OBJECTS,
				marker: crate::textobj::TextObjects,
				runtime_ty: crate::textobj::registry::TextObjectRegistry,
				init: crate::textobj::registry::TextObjectRegistry::new($indices.text_objects),
			}
			{
				field: options,
				global: OPTIONS,
				marker: crate::options::Options,
				runtime_ty: crate::options::registry::OptionsRegistry,
				init: crate::options::registry::OptionsRegistry::new($indices.options),
			}
			#[cfg(feature = "commands")]
			{
				field: snippets,
				global: SNIPPETS,
				marker: crate::snippets::Snippets,
				runtime_ty: crate::core::RuntimeRegistry<crate::snippets::entry::SnippetEntry, crate::core::SnippetId>,
				init: crate::core::RuntimeRegistry::new("snippets", $indices.snippets),
			}
			{
				field: themes,
				global: THEMES,
				marker: crate::themes::Themes,
				runtime_ty: crate::core::RuntimeRegistry<crate::themes::theme::ThemeEntry, crate::core::ThemeId>,
				init: crate::core::RuntimeRegistry::new("themes", $indices.themes),
			}
			{
				field: gutters,
				global: GUTTERS,
				marker: crate::gutter::Gutters,
				runtime_ty: crate::core::RuntimeRegistry<crate::gutter::GutterEntry, crate::core::GutterId>,
				init: crate::core::RuntimeRegistry::new("gutters", $indices.gutters),
			}
			{
				field: statusline,
				global: STATUSLINE_SEGMENTS,
				marker: crate::statusline::Statusline,
				runtime_ty: crate::core::RuntimeRegistry<crate::statusline::StatuslineEntry, crate::core::StatuslineId>,
				init: crate::core::RuntimeRegistry::new("statusline", $indices.statusline),
			}
			{
				field: hooks,
				global: HOOKS,
				marker: crate::hooks::Hooks,
				runtime_ty: crate::hooks::registry::HooksRegistry,
				init: crate::hooks::registry::HooksRegistry::new($indices.hooks),
			}
			{
				field: notifications,
				global: NOTIFICATIONS,
				marker: crate::notifications::Notifications,
				runtime_ty: crate::core::RuntimeRegistry<crate::notifications::NotificationEntry, crate::notifications::NotificationId>,
				init: crate::core::RuntimeRegistry::new("notifications", $indices.notifications),
			}
			{
				field: languages,
				global: LANGUAGES,
				marker: crate::languages::Languages,
				runtime_ty: crate::languages::LanguagesRegistry,
				init: crate::languages::LanguagesRegistry::new($indices.languages),
			}
			{
				field: lsp_servers,
				global: LSP_SERVERS,
				marker: crate::lsp_servers::LspServers,
				runtime_ty: crate::lsp_servers::LspServersRegistry,
				init: crate::lsp_servers::LspServersRegistry::new($indices.lsp_servers),
			}
		}
	};
}

pub(crate) use with_registry_domains;
