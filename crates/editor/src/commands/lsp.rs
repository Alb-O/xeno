//! LSP commands with direct [`Editor`] access.

use xeno_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, MarkedString, MarkupContent};
use xeno_primitives::LocalBoxFuture;
use xeno_registry::Capability;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::impls::Editor;
use crate::info_popup::PopupAnchor;

editor_command!(
	hover,
	{ aliases: &["lsp-hover"], description: "Show hover information at cursor" },
	handler: cmd_hover
);

fn cmd_hover<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let hover = ctx
			.editor
			.lsp()
			.hover(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("No hover information available".into()))?;

		let content = format_hover_contents(&hover.contents);
		Editor::open_info_popup(ctx.editor, content, Some("markdown"), PopupAnchor::Center);
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	gd,
	{
		aliases: &["goto-definition", "lsp-definition"],
		description: "Go to definition",
		caps: &[Capability::FileOps]
	},
	handler: cmd_goto_definition
);

fn cmd_goto_definition<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		use crate::impls::Location;

		let response = ctx
			.editor
			.lsp()
			.goto_definition(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("No definition found".into()))?;

		let location = match response {
			GotoDefinitionResponse::Scalar(loc) => loc,
			GotoDefinitionResponse::Array(locs) => locs
				.into_iter()
				.next()
				.ok_or_else(|| CommandError::Failed("No definition found".into()))?,
			GotoDefinitionResponse::Link(links) => {
				let link = links
					.into_iter()
					.next()
					.ok_or_else(|| CommandError::Failed("No definition found".into()))?;
				xeno_lsp::lsp_types::Location {
					uri: link.target_uri,
					range: link.target_selection_range,
				}
			}
		};

		let path = xeno_lsp::path_from_uri(&location.uri)
			.ok_or_else(|| CommandError::Failed("Invalid file path in definition".into()))?;

		ctx.editor
			.goto_location(&Location::from_lsp(path, &location.range.start))
			.await
			.map_err(|e| CommandError::Io(e.to_string()))?;

		Ok(CommandOutcome::Ok)
	})
}

/// Formats LSP hover contents to markdown.
fn format_hover_contents(contents: &HoverContents) -> String {
	match contents {
		HoverContents::Scalar(MarkedString::String(s)) => s.clone(),
		HoverContents::Scalar(MarkedString::LanguageString(ls)) => {
			format!("```{}\n{}\n```", ls.language, ls.value)
		}
		HoverContents::Array(parts) => parts
			.iter()
			.map(|p| match p {
				MarkedString::String(s) => s.clone(),
				MarkedString::LanguageString(ls) => {
					format!("```{}\n{}\n```", ls.language, ls.value)
				}
			})
			.collect::<Vec<_>>()
			.join("\n\n"),
		HoverContents::Markup(MarkupContent { value, .. }) => value.clone(),
	}
}

editor_command!(
	code_action,
	{
		aliases: &["code-action", "code-actions", "lsp-code-action", "lsp-code-actions"],
		description: "Show code actions at cursor",
		caps: &[Capability::Edit]
	},
	handler: cmd_code_action
);

fn cmd_code_action<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_code_action_menu().await;
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	rename,
	{
		aliases: &["lsp-rename"],
		description: "Rename symbol at cursor",
		caps: &[Capability::Edit]
	},
	handler: cmd_rename
);

fn cmd_rename<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_rename();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	diagnostic_next,
	{
		aliases: &["diagnostic-next", "diag-next", "lsp-diagnostic-next"],
		description: "Jump to next diagnostic"
	},
	handler: cmd_diagnostic_next
);

fn cmd_diagnostic_next<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.goto_next_diagnostic();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	diagnostic_prev,
	{
		aliases: &["diagnostic-prev", "diag-prev", "lsp-diagnostic-prev"],
		description: "Jump to previous diagnostic"
	},
	handler: cmd_diagnostic_prev
);

fn cmd_diagnostic_prev<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.goto_prev_diagnostic();
		Ok(CommandOutcome::Ok)
	})
}
