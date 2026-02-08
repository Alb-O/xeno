//! LSP commands with direct [`Editor`] access.

use xeno_primitives::BoxFutureLocal;
use xeno_registry::Capability;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor_command;
use crate::impls::Editor;
use crate::info_popup::PopupAnchor;

editor_command!(
	hover,
	{ keys: &["lsp-hover"], description: "Show hover information at cursor" },
	handler: cmd_hover
);

fn cmd_hover<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
		keys: &["goto-definition", "lsp-definition"],
		description: "Go to definition",
		caps: &[Capability::FileOps]
	},
	handler: cmd_goto_definition
);

fn cmd_goto_definition<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
			xeno_lsp::lsp_types::GotoDefinitionResponse::Scalar(loc) => loc,
			xeno_lsp::lsp_types::GotoDefinitionResponse::Array(locs) => locs
				.into_iter()
				.next()
				.ok_or_else(|| CommandError::Failed("No definition found".into()))?,
			xeno_lsp::lsp_types::GotoDefinitionResponse::Link(links) => {
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
			.goto_location(&Location::new(
				path,
				location.range.start.line as usize,
				location.range.start.character as usize,
			))
			.await
			.map_err(|e| CommandError::Io(e.to_string()))?;

		Ok(CommandOutcome::Ok)
	})
}

/// Formats a single [`MarkedString`] to markdown.
fn format_marked_string(ms: &xeno_lsp::lsp_types::MarkedString) -> String {
	match ms {
		xeno_lsp::lsp_types::MarkedString::String(s) => s.clone(),
		xeno_lsp::lsp_types::MarkedString::LanguageString(ls) => {
			format!("```{}\n{}\n```", ls.language, ls.value)
		}
	}
}

/// Formats LSP hover contents to markdown.
fn format_hover_contents(contents: &xeno_lsp::lsp_types::HoverContents) -> String {
	match contents {
		xeno_lsp::lsp_types::HoverContents::Scalar(ms) => format_marked_string(ms),
		xeno_lsp::lsp_types::HoverContents::Array(parts) => parts
			.iter()
			.map(format_marked_string)
			.collect::<Vec<_>>()
			.join("\n\n"),
		xeno_lsp::lsp_types::HoverContents::Markup(m) => m.value.clone(),
	}
}

editor_command!(
	code_action,
	{
		keys: &["code-action", "code-actions", "lsp-code-action", "lsp-code-actions"],
		description: "Show code actions at cursor",
		caps: &[Capability::Edit]
	},
	handler: cmd_code_action
);

fn cmd_code_action<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_code_action_menu().await;
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	rename,
	{
		keys: &["lsp-rename"],
		description: "Rename symbol at cursor",
		caps: &[Capability::Edit]
	},
	handler: cmd_rename
);

fn cmd_rename<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_rename();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	diagnostic_next,
	{
		keys: &["diagnostic-next", "diag-next", "lsp-diagnostic-next"],
		description: "Jump to next diagnostic"
	},
	handler: cmd_diagnostic_next
);

fn cmd_diagnostic_next<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.goto_next_diagnostic();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	diagnostic_prev,
	{
		keys: &["diagnostic-prev", "diag-prev", "lsp-diagnostic-prev"],
		description: "Jump to previous diagnostic"
	},
	handler: cmd_diagnostic_prev
);

fn cmd_diagnostic_prev<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.goto_prev_diagnostic();
		Ok(CommandOutcome::Ok)
	})
}
