//! LSP commands with direct [`Editor`] access.

use xeno_primitives::BoxFutureLocal;

use super::{CommandError, CommandOutcome, EditorCommandContext};
use crate::info_popup::PopupAnchor;
use crate::lsp::types::{LspMenuKind, LspMenuState};
use crate::{Editor, editor_command};

editor_command!(
	hover,
	{ keys: &["lsp-hover"], description: "Show hover information at cursor" },
	handler: cmd_hover
);

fn cmd_hover<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
		mutates_buffer: false
	},
	handler: cmd_goto_definition
);

fn cmd_goto_definition<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.goto_definition(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?;
		handle_goto_response(ctx.editor, response, encoding, "definition").await
	})
}

editor_command!(
	goto_declaration,
	{
		keys: &["goto-declaration", "lsp-declaration"],
		description: "Go to declaration",
		mutates_buffer: false
	},
	handler: cmd_goto_declaration
);

fn cmd_goto_declaration<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.goto_declaration(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?;
		handle_goto_response(ctx.editor, response, encoding, "declaration").await
	})
}

editor_command!(
	goto_implementation,
	{
		keys: &["goto-implementation", "lsp-implementation"],
		description: "Go to implementation",
		mutates_buffer: false
	},
	handler: cmd_goto_implementation
);

fn cmd_goto_implementation<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.goto_implementation(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?;
		handle_goto_response(ctx.editor, response, encoding, "implementation").await
	})
}

editor_command!(
	goto_type_definition,
	{
		keys: &["goto-type-definition", "lsp-type-definition"],
		description: "Go to type definition",
		mutates_buffer: false
	},
	handler: cmd_goto_type_definition
);

fn cmd_goto_type_definition<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.goto_type_definition(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?;
		handle_goto_response(ctx.editor, response, encoding, "type definition").await
	})
}

/// Shared handler for goto-family responses (definition, declaration, implementation, type definition).
///
/// Normalizes the response into locations, then: single → jump, multiple → picker, none → error.
async fn handle_goto_response(
	editor: &mut Editor,
	response: Option<xeno_lsp::lsp_types::GotoDefinitionResponse>,
	encoding: xeno_lsp::OffsetEncoding,
	kind: &str,
) -> Result<CommandOutcome, CommandError> {
	let response = response.ok_or_else(|| CommandError::Failed(format!("No {kind} found")))?;

	let locations = goto_response_to_locations(response);
	if locations.is_empty() {
		return Err(CommandError::Failed(format!("No {kind} found")));
	}

	if locations.len() == 1 {
		editor
			.goto_lsp_location(&locations[0], encoding)
			.await
			.map_err(|e| CommandError::Io(e.to_string()))?;
		return Ok(CommandOutcome::Ok);
	}

	editor.open_locations_menu(locations, encoding);
	Ok(CommandOutcome::Ok)
}

/// Converts a GotoDefinitionResponse into a flat list of locations.
fn goto_response_to_locations(response: xeno_lsp::lsp_types::GotoDefinitionResponse) -> Vec<xeno_lsp::lsp_types::Location> {
	match response {
		xeno_lsp::lsp_types::GotoDefinitionResponse::Scalar(loc) => vec![loc],
		xeno_lsp::lsp_types::GotoDefinitionResponse::Array(locs) => locs,
		xeno_lsp::lsp_types::GotoDefinitionResponse::Link(links) => links
			.into_iter()
			.map(|link| xeno_lsp::lsp_types::Location {
				uri: link.target_uri,
				range: link.target_selection_range,
			})
			.collect(),
	}
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
		xeno_lsp::lsp_types::HoverContents::Array(parts) => parts.iter().map(format_marked_string).collect::<Vec<_>>().join("\n\n"),
		xeno_lsp::lsp_types::HoverContents::Markup(m) => m.value.clone(),
	}
}

editor_command!(
	code_action,
	{
		keys: &["code-action", "code-actions", "lsp-code-action", "lsp-code-actions"],
		description: "Show code actions at cursor",
		mutates_buffer: true
	},
	handler: cmd_code_action
);

fn cmd_code_action<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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
		mutates_buffer: true
	},
	handler: cmd_rename
);

fn cmd_rename<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.open_rename();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	references,
	{
		keys: &["lsp-references", "references"],
		description: "Find all references to symbol at cursor"
	},
	handler: cmd_references
);

fn cmd_references<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let locations = ctx
			.editor
			.lsp()
			.references(ctx.editor.buffer(), true)
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("No references found".into()))?;

		if locations.is_empty() {
			return Err(CommandError::Failed("No references found".into()));
		}

		if locations.len() == 1 {
			ctx.editor
				.goto_lsp_location(&locations[0], encoding)
				.await
				.map_err(|e| CommandError::Io(e.to_string()))?;
			return Ok(CommandOutcome::Ok);
		}

		ctx.editor.open_locations_menu(locations, encoding);
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	document_symbols,
	{
		keys: &["lsp-symbols", "lsp-outline", "symbols"],
		description: "Show document symbol outline"
	},
	handler: cmd_document_symbols
);

fn cmd_document_symbols<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.document_symbol(ctx.editor.buffer())
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("No symbols found".into()))?;

		let (labels, locations) = flatten_document_symbols(&response, ctx.editor.buffer());
		if labels.is_empty() {
			return Err(CommandError::Failed("No symbols found".into()));
		}

		ctx.editor.open_symbols_menu(labels, locations, encoding);
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	workspace_symbol,
	{
		keys: &["workspace-symbol", "lsp-workspace-symbol"],
		description: "Search workspace symbols"
	},
	handler: cmd_workspace_symbol
);

fn cmd_workspace_symbol<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let query = if ctx.args.is_empty() {
			let buffer = ctx.editor.buffer();
			let sel = buffer.selection.primary();
			if sel.is_point() {
				String::new()
			} else {
				buffer.with_doc(|doc| doc.content().slice(sel.from()..sel.to()).to_string())
			}
		} else {
			ctx.args.join(" ")
		};

		if query.is_empty() {
			return Err(CommandError::Failed("No query provided".into()));
		}

		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(ctx.editor.buffer());
		let response = ctx
			.editor
			.lsp()
			.workspace_symbol(ctx.editor.buffer(), query)
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("Workspace symbols not supported".into()))?;

		let (labels, locations) = flatten_workspace_symbols(&response);
		if labels.is_empty() {
			return Err(CommandError::Failed("No symbols found".into()));
		}

		ctx.editor.open_locations_menu(locations, encoding);
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	format,
	{
		keys: &["lsp-format", "format"],
		description: "Format document via LSP",
		mutates_buffer: true
	},
	handler: cmd_format
);

fn cmd_format<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let tab_width = ctx.editor.tab_width() as u32;
		let options = xeno_lsp::lsp_types::FormattingOptions {
			tab_size: tab_width,
			insert_spaces: false,
			..Default::default()
		};
		let edits = ctx
			.editor
			.lsp()
			.formatting(ctx.editor.buffer(), options)
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("Formatting not supported".into()))?;

		if edits.is_empty() {
			return Ok(CommandOutcome::Ok);
		}

		let uri = ctx
			.editor
			.buffer()
			.path()
			.and_then(|p| xeno_lsp::uri_from_path(&ctx.editor.lsp().canonicalize_path(&p)))
			.ok_or_else(|| CommandError::Failed("Buffer has no file path".into()))?;

		let workspace_edit = xeno_lsp::lsp_types::WorkspaceEdit {
			changes: Some([(uri, edits)].into_iter().collect()),
			..Default::default()
		};

		ctx.editor
			.apply_workspace_edit(workspace_edit)
			.await
			.map_err(|e| CommandError::Failed(e.error.to_string()))?;

		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	format_selection,
	{
		keys: &["format-selection", "lsp-format-selection"],
		description: "Format selection via LSP",
		mutates_buffer: true
	},
	handler: cmd_format_selection
);

fn cmd_format_selection<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let tab_width = ctx.editor.tab_width() as u32;
		let options = xeno_lsp::lsp_types::FormattingOptions {
			tab_size: tab_width,
			insert_spaces: false,
			..Default::default()
		};

		let buffer = ctx.editor.buffer();
		let selection = buffer.selection.primary();
		let start = if selection.is_point() { buffer.cursor } else { selection.from() };
		let end = if selection.is_point() { buffer.cursor } else { selection.to() };
		let encoding = ctx.editor.lsp().offset_encoding_for_buffer(buffer);

		let range = buffer
			.with_doc(|doc| xeno_lsp::char_range_to_lsp_range(doc.content(), start, end, encoding))
			.ok_or_else(|| CommandError::Failed("Invalid selection range".into()))?;

		let edits = ctx
			.editor
			.lsp()
			.range_formatting(ctx.editor.buffer(), range, options)
			.await
			.map_err(|e| CommandError::Failed(e.to_string()))?
			.ok_or_else(|| CommandError::Failed("Range formatting not supported".into()))?;

		if edits.is_empty() {
			return Ok(CommandOutcome::Ok);
		}

		let uri = ctx
			.editor
			.buffer()
			.path()
			.and_then(|p| xeno_lsp::uri_from_path(&ctx.editor.lsp().canonicalize_path(&p)))
			.ok_or_else(|| CommandError::Failed("Buffer has no file path".into()))?;

		let workspace_edit = xeno_lsp::lsp_types::WorkspaceEdit {
			changes: Some([(uri, edits)].into_iter().collect()),
			..Default::default()
		};

		ctx.editor
			.apply_workspace_edit(workspace_edit)
			.await
			.map_err(|e| CommandError::Failed(e.error.to_string()))?;

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

fn cmd_diagnostic_next<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
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

fn cmd_diagnostic_prev<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.goto_prev_diagnostic();
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	rename_file,
	{
		keys: &["rename-file", "move-file"],
		description: "Rename/move the current file on disk",
		mutates_buffer: true
	},
	handler: cmd_rename_file
);

fn cmd_rename_file<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let new_name = ctx
			.args
			.first()
			.ok_or_else(|| CommandError::InvalidArgument("Usage: rename-file <new-path>".into()))?;
		let new_path = std::path::PathBuf::from(new_name);

		ctx.editor.rename_current_file(new_path).await?;
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	create_file,
	{
		keys: &["create-file", "new-file", "touch"],
		description: "Create a new file on disk and open it",
		mutates_buffer: true
	},
	handler: cmd_create_file
);

fn cmd_create_file<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		let name = ctx
			.args
			.first()
			.ok_or_else(|| CommandError::InvalidArgument("Usage: create-file <path>".into()))?;
		let path = std::path::PathBuf::from(name);

		ctx.editor.create_file(path).await?;
		Ok(CommandOutcome::Ok)
	})
}

editor_command!(
	delete_file,
	{
		keys: &["delete-file", "rm"],
		description: "Delete the current file from disk",
		mutates_buffer: true
	},
	handler: cmd_delete_file
);

fn cmd_delete_file<'a>(ctx: &'a mut EditorCommandContext<'a>) -> BoxFutureLocal<'a, Result<CommandOutcome, CommandError>> {
	Box::pin(async move {
		ctx.editor.delete_current_file().await?;
		Ok(CommandOutcome::Ok)
	})
}

impl Editor {
	fn open_locations_menu(&mut self, locations: Vec<xeno_lsp::lsp_types::Location>, encoding: xeno_lsp::OffsetEncoding) {
		use crate::completion::{CompletionItem, CompletionState};
		use crate::render_api::CompletionKind;

		let buffer_id = self.focused_view();
		let display_items: Vec<CompletionItem> = locations
			.iter()
			.map(|loc| {
				let path = xeno_lsp::path_from_uri(&loc.uri)
					.map(|p| p.display().to_string())
					.unwrap_or_else(|| loc.uri.to_string());
				let line = loc.range.start.line + 1;
				let col = loc.range.start.character + 1;
				let label = format!("{path}:{line}:{col}");
				CompletionItem {
					label: label.clone(),
					insert_text: label,
					detail: None,
					filter_text: None,
					kind: CompletionKind::Command,
					match_indices: None,
					right: None,
					file: None,
				}
			})
			.collect();

		let completions = self.overlays_mut().get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.selected_idx = Some(0);
		completions.active = true;
		completions.replace_start = 0;
		completions.scroll_offset = 0;

		let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
		menu_state.set(LspMenuKind::References {
			buffer_id,
			locations,
			encoding,
		});

		self.state.core.frame.needs_redraw = true;
	}

	fn open_symbols_menu(&mut self, labels: Vec<String>, locations: Vec<xeno_lsp::lsp_types::Location>, encoding: xeno_lsp::OffsetEncoding) {
		use crate::completion::{CompletionItem, CompletionState};
		use crate::render_api::CompletionKind;

		let buffer_id = self.focused_view();
		let display_items: Vec<CompletionItem> = labels
			.into_iter()
			.map(|label| CompletionItem {
				label: label.clone(),
				insert_text: label,
				detail: None,
				filter_text: None,
				kind: CompletionKind::Command,
				match_indices: None,
				right: None,
				file: None,
			})
			.collect();

		let completions = self.overlays_mut().get_or_default::<CompletionState>();
		completions.items = display_items;
		completions.selected_idx = Some(0);
		completions.active = true;
		completions.replace_start = 0;
		completions.scroll_offset = 0;

		let menu_state = self.overlays_mut().get_or_default::<LspMenuState>();
		menu_state.set(LspMenuKind::Symbols {
			buffer_id,
			locations,
			encoding,
		});

		self.state.core.frame.needs_redraw = true;
	}
}

/// Flattens a document symbol response into parallel label + location vectors.
fn flatten_document_symbols(
	response: &xeno_lsp::lsp_types::DocumentSymbolResponse,
	buffer: &crate::buffer::Buffer,
) -> (Vec<String>, Vec<xeno_lsp::lsp_types::Location>) {
	let uri = buffer
		.path()
		.and_then(|p| xeno_lsp::uri_from_path(&p))
		.unwrap_or_else(|| "file:///unknown".parse().unwrap());

	let mut labels = Vec::new();
	let mut locations = Vec::new();

	match response {
		xeno_lsp::lsp_types::DocumentSymbolResponse::Flat(symbols) => {
			for sym in symbols {
				let line = sym.location.range.start.line + 1;
				let kind = format_symbol_kind(sym.kind);
				labels.push(format!("{kind:>12}  {}  :{line}", sym.name));
				locations.push(sym.location.clone());
			}
		}
		xeno_lsp::lsp_types::DocumentSymbolResponse::Nested(symbols) => {
			flatten_nested_symbols(&uri, symbols, 0, &mut labels, &mut locations);
		}
	}

	(labels, locations)
}

fn flatten_nested_symbols(
	uri: &xeno_lsp::lsp_types::Uri,
	symbols: &[xeno_lsp::lsp_types::DocumentSymbol],
	depth: usize,
	labels: &mut Vec<String>,
	locations: &mut Vec<xeno_lsp::lsp_types::Location>,
) {
	let indent = "  ".repeat(depth);
	for sym in symbols {
		let line = sym.range.start.line + 1;
		let kind = format_symbol_kind(sym.kind);
		labels.push(format!("{indent}{kind:>12}  {}  :{line}", sym.name));
		locations.push(xeno_lsp::lsp_types::Location {
			uri: uri.clone(),
			range: sym.selection_range,
		});
		if let Some(children) = &sym.children {
			flatten_nested_symbols(uri, children, depth + 1, labels, locations);
		}
	}
}

/// Flattens a workspace symbol response into parallel label + location vectors.
fn flatten_workspace_symbols(response: &xeno_lsp::lsp_types::WorkspaceSymbolResponse) -> (Vec<String>, Vec<xeno_lsp::lsp_types::Location>) {
	let mut labels = Vec::new();
	let mut locations = Vec::new();

	match response {
		xeno_lsp::lsp_types::WorkspaceSymbolResponse::Flat(symbols) => {
			for sym in symbols {
				let kind = format_symbol_kind(sym.kind);
				let container = sym.container_name.as_deref().unwrap_or("");
				let label = if container.is_empty() {
					format!("{kind:>12}  {}", sym.name)
				} else {
					format!("{kind:>12}  {}  ({container})", sym.name)
				};
				labels.push(label);
				locations.push(sym.location.clone());
			}
		}
		xeno_lsp::lsp_types::WorkspaceSymbolResponse::Nested(symbols) => {
			for sym in symbols {
				let kind = format_symbol_kind(sym.kind);
				let container = sym.container_name.as_deref().unwrap_or("");
				let label = if container.is_empty() {
					format!("{kind:>12}  {}", sym.name)
				} else {
					format!("{kind:>12}  {}  ({container})", sym.name)
				};
				labels.push(label);
				// WorkspaceSymbol uses Location or { uri } — extract location
				let location = match &sym.location {
					xeno_lsp::lsp_types::OneOf::Left(loc) => loc.clone(),
					xeno_lsp::lsp_types::OneOf::Right(uri_only) => xeno_lsp::lsp_types::Location {
						uri: uri_only.uri.clone(),
						range: Default::default(),
					},
				};
				locations.push(location);
			}
		}
	}

	(labels, locations)
}

fn format_symbol_kind(kind: xeno_lsp::lsp_types::SymbolKind) -> &'static str {
	use xeno_lsp::lsp_types::SymbolKind;
	match kind {
		SymbolKind::FILE => "file",
		SymbolKind::MODULE => "mod",
		SymbolKind::NAMESPACE => "ns",
		SymbolKind::PACKAGE => "pkg",
		SymbolKind::CLASS => "class",
		SymbolKind::METHOD => "method",
		SymbolKind::PROPERTY => "prop",
		SymbolKind::FIELD => "field",
		SymbolKind::CONSTRUCTOR => "ctor",
		SymbolKind::ENUM => "enum",
		SymbolKind::INTERFACE => "iface",
		SymbolKind::FUNCTION => "fn",
		SymbolKind::VARIABLE => "var",
		SymbolKind::CONSTANT => "const",
		SymbolKind::STRING => "str",
		SymbolKind::NUMBER => "num",
		SymbolKind::BOOLEAN => "bool",
		SymbolKind::ARRAY => "array",
		SymbolKind::OBJECT => "obj",
		SymbolKind::KEY => "key",
		SymbolKind::NULL => "null",
		SymbolKind::ENUM_MEMBER => "variant",
		SymbolKind::STRUCT => "struct",
		SymbolKind::EVENT => "event",
		SymbolKind::OPERATOR => "op",
		SymbolKind::TYPE_PARAMETER => "typaram",
		_ => "?",
	}
}
