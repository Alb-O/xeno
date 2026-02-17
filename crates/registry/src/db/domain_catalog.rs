//! Shared domain wiring catalog used by both builder and runtime DB assembly.
//!
//! This is the single source of truth for domain field names, marker types,
//! and global accessor names.

macro_rules! with_registry_domains {
	($callback:ident) => {
		$callback! {
			{
				field: actions,
				global: ACTIONS,
				marker: crate::actions::Actions,
			}
			{
				field: commands,
				global: COMMANDS,
				marker: crate::commands::Commands,
			}
			{
				field: motions,
				global: MOTIONS,
				marker: crate::motions::Motions,
			}
			{
				field: text_objects,
				global: TEXT_OBJECTS,
				marker: crate::textobj::TextObjects,
			}
			{
				field: options,
				global: OPTIONS,
				marker: crate::options::Options,
			}
			#[cfg(feature = "commands")]
			{
				field: snippets,
				global: SNIPPETS,
				marker: crate::snippets::Snippets,
			}
			{
				field: themes,
				global: THEMES,
				marker: crate::themes::Themes,
			}
			{
				field: gutters,
				global: GUTTERS,
				marker: crate::gutter::Gutters,
			}
			{
				field: statusline,
				global: STATUSLINE_SEGMENTS,
				marker: crate::statusline::Statusline,
			}
			{
				field: hooks,
				global: HOOKS,
				marker: crate::hooks::Hooks,
			}
			{
				field: notifications,
				global: NOTIFICATIONS,
				marker: crate::notifications::Notifications,
			}
			{
				field: languages,
				global: LANGUAGES,
				marker: crate::languages::Languages,
			}
			{
				field: lsp_servers,
				global: LSP_SERVERS,
				marker: crate::lsp_servers::LspServers,
			}
		}
	};
}

pub(crate) use with_registry_domains;
