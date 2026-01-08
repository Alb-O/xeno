//! LSP commands with direct [`Editor`] access.

use futures::future::LocalBoxFuture;
use xeno_lsp::lsp_types::{GotoDefinitionResponse, HoverContents, MarkedString, MarkupContent};

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::editor::Editor;
use crate::editor_command;
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
			.lsp
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
	{ aliases: &["goto-definition", "lsp-definition"], description: "Go to definition" },
	handler: cmd_goto_definition
);

fn cmd_goto_definition<'a>(
	ctx: &'a mut EditorCommandContext<'a>,
) -> LocalBoxFuture<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		use crate::editor::Location;

		let response = ctx
			.editor
			.lsp
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
