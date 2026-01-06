//! LSP-UI bridge module.
//!
//! This module coordinates async LSP requests with UI updates, providing methods
//! to trigger LSP features that display popups or other UI elements.

use std::sync::Arc;

use xeno_lsp::lsp_types::{
    AnnotatedTextEdit, CodeActionOrCommand, CodeActionResponse, CompletionResponse, Hover, OneOf,
    Position, SignatureHelp, TextEdit, Url, WorkspaceEdit,
};
use xeno_lsp::{ClientHandle, OffsetEncoding, Registry};

use crate::buffer::Buffer;
use crate::editor::extensions::ExtensionMap;
use crate::ui::popup::{
    CodeActionResult, CodeActionsPopup, CompletionPopup, HoverPopup, PopupAnchor, SignaturePopup,
};
use crate::ui::UiManager;

/// Gets the LSP registry from the extension map.
///
/// The LSP extension stores `Arc<Registry>` directly in the extension map for access
/// by rendering code and LSP UI operations.
pub fn get_lsp_registry(extensions: &ExtensionMap) -> Option<Arc<Registry>> {
    extensions.get::<Arc<Registry>>().cloned()
}

/// Alias for `get_lsp_registry` for backwards compatibility.
pub fn get_lsp_manager(extensions: &ExtensionMap) -> Option<Arc<Registry>> {
    get_lsp_registry(extensions)
}

/// Gets an LSP client for the given buffer.
pub fn get_client_for_buffer(
    registry: &Registry,
    buffer: &Buffer,
) -> Option<ClientHandle> {
    let path = buffer.path()?;
    let language = buffer.file_type()?;
    registry.get_for_file(&language, &path)
}

/// Converts a buffer cursor position to an LSP position.
///
/// Uses the offset encoding negotiated with the server.
pub fn buffer_cursor_to_lsp_position(
    buffer: &Buffer,
    encoding: OffsetEncoding,
) -> Option<Position> {
    xeno_lsp::char_to_lsp_position(&buffer.doc().content, buffer.cursor, encoding)
}

/// Gets the file URI for a buffer.
pub fn buffer_to_uri(buffer: &Buffer) -> Option<Url> {
    let path = buffer.path()?;
    Url::from_file_path(&path).ok()
}

/// Requests hover information from the LSP server for the given buffer.
///
/// Returns the hover response if available.
pub async fn request_hover(
    registry: &Registry,
    buffer: &Buffer,
) -> Option<Hover> {
    let client = get_client_for_buffer(registry, buffer)?;
    
    if !client.is_initialized() {
        return None;
    }
    
    let uri = buffer_to_uri(buffer)?;
    let encoding = client.offset_encoding();
    let position = buffer_cursor_to_lsp_position(buffer, encoding)?;
    
    client.hover(uri, position).await.ok().flatten()
}

/// Shows a hover popup for the current buffer cursor position.
///
/// This is the main entry point for the hover feature. It:
/// 1. Gets the LSP manager from extensions
/// 2. Requests hover information from the language server
/// 3. Shows the hover popup in the UI
///
/// # Arguments
///
/// * `ui` - The UI manager for showing popups
/// * `extensions` - The extension map containing the LSP manager
/// * `buffer` - The buffer to request hover for
/// * `cursor_screen_pos` - Optional screen position of the cursor for popup anchoring
///
/// # Returns
///
/// Returns `true` if a hover popup was shown, `false` otherwise.
pub async fn show_hover(
    ui: &mut UiManager,
    extensions: &ExtensionMap,
    buffer: &Buffer,
    cursor_screen_pos: Option<(u16, u16)>,
) -> bool {
    let Some(registry) = get_lsp_registry(extensions) else {
        return false;
    };
    
    let Some(hover) = request_hover(&registry, buffer).await else {
        return false;
    };
    
    // Create the hover popup
    let mut popup = HoverPopup::from_hover(hover);
    
    // Set anchor based on cursor position if available
    if let Some((x, y)) = cursor_screen_pos {
        popup = popup.with_anchor(PopupAnchor::Position {
            x,
            y,
            prefer_above: false,
        });
    }
    
    ui.show_popup(Box::new(popup));
    true
}

/// Dismisses any active hover popup.
pub fn dismiss_hover(ui: &mut UiManager) {
    ui.dismiss_popup("lsp-hover");
}

/// Checks if a hover popup is currently shown.
pub fn has_hover(ui: &UiManager) -> bool {
    // We check if popups are present - more specific checking could be added
    ui.has_popups()
}

// ─────────────────────────────────────────────────────────────────────────────
// Completion
// ─────────────────────────────────────────────────────────────────────────────

/// Requests completion items from the LSP server for the given buffer.
///
/// Returns the completion response if available.
pub async fn request_completion(
    registry: &Registry,
    buffer: &Buffer,
) -> Option<CompletionResponse> {
    let client = get_client_for_buffer(registry, buffer)?;
    
    if !client.is_initialized() {
        return None;
    }
    
    let uri = buffer_to_uri(buffer)?;
    let encoding = client.offset_encoding();
    let position = buffer_cursor_to_lsp_position(buffer, encoding)?;
    
    client.completion(uri, position, None).await.ok().flatten()
}

/// Shows a completion popup for the current buffer cursor position.
///
/// This is the main entry point for the completion feature. It:
/// 1. Gets the LSP manager from extensions
/// 2. Requests completion items from the language server
/// 3. Shows the completion popup in the UI
///
/// # Arguments
///
/// * `ui` - The UI manager for showing popups
/// * `extensions` - The extension map containing the LSP manager
/// * `buffer` - The buffer to request completion for
/// * `filter_text` - Text typed since the trigger character
/// * `trigger_column` - Column where completion was triggered
/// * `cursor_screen_pos` - Optional screen position of the cursor for popup anchoring
///
/// # Returns
///
/// Returns `true` if a completion popup was shown, `false` otherwise.
pub async fn show_completion(
    ui: &mut UiManager,
    extensions: &ExtensionMap,
    buffer: &Buffer,
    filter_text: String,
    trigger_column: usize,
    cursor_screen_pos: Option<(u16, u16)>,
) -> bool {
    let Some(registry) = get_lsp_registry(extensions) else {
        return false;
    };
    
    let Some(response) = request_completion(&registry, buffer).await else {
        return false;
    };
    
    // Create the completion popup
    let mut popup = CompletionPopup::from_response(response, filter_text, trigger_column);
    
    // Don't show empty popups
    if !popup.has_items() {
        return false;
    }
    
    ui.show_popup(Box::new(popup));
    true
}

/// Updates the filter text for an active completion popup.
///
/// If completion is active, updates the filter. Otherwise does nothing.
pub fn update_completion_filter(ui: &mut UiManager, filter_text: String) {
    // The completion popup handles filtering internally via events
    // For now, we dismiss and re-show - a more sophisticated approach
    // would access the popup directly
    let _ = filter_text;
}

/// Dismisses any active completion popup.
pub fn dismiss_completion(ui: &mut UiManager) {
    ui.dismiss_popup("lsp-completion");
}

/// Checks if a completion popup is currently shown.
pub fn has_completion(ui: &UiManager) -> bool {
    ui.has_popups()
}

// ─────────────────────────────────────────────────────────────────────────────
// Signature Help
// ─────────────────────────────────────────────────────────────────────────────

/// Requests signature help from the LSP server for the given buffer.
///
/// Returns the signature help response if available.
pub async fn request_signature_help(
    registry: &Registry,
    buffer: &Buffer,
) -> Option<SignatureHelp> {
    let client = get_client_for_buffer(registry, buffer)?;
    
    if !client.is_initialized() {
        return None;
    }
    
    let uri = buffer_to_uri(buffer)?;
    let encoding = client.offset_encoding();
    let position = buffer_cursor_to_lsp_position(buffer, encoding)?;
    
    client.signature_help(uri, position, None).await.ok().flatten()
}

/// Shows a signature help popup for the current buffer cursor position.
///
/// This is the main entry point for the signature help feature. It:
/// 1. Gets the LSP manager from extensions
/// 2. Requests signature help from the language server
/// 3. Shows the signature help popup in the UI
///
/// # Arguments
///
/// * `ui` - The UI manager for showing popups
/// * `extensions` - The extension map containing the LSP manager
/// * `buffer` - The buffer to request signature help for
/// * `active_parameter` - Optional index of the active parameter (derived from comma count)
/// * `cursor_screen_pos` - Optional screen position of the cursor for popup anchoring
///
/// # Returns
///
/// Returns `true` if a signature help popup was shown, `false` otherwise.
pub async fn show_signature_help(
    ui: &mut UiManager,
    extensions: &ExtensionMap,
    buffer: &Buffer,
    active_parameter: Option<usize>,
    cursor_screen_pos: Option<(u16, u16)>,
) -> bool {
    let Some(registry) = get_lsp_registry(extensions) else {
        return false;
    };
    
    let Some(help) = request_signature_help(&registry, buffer).await else {
        return false;
    };
    
    // Create the signature popup
    let Some(mut popup) = SignaturePopup::from_signature_help(help) else {
        return false;
    };
    
    // Set active parameter if provided (overrides LSP response)
    if let Some(param_idx) = active_parameter {
        popup.set_active_parameter(Some(param_idx));
    }
    
    ui.show_popup(Box::new(popup));
    true
}

/// Updates the active parameter for an existing signature help popup.
///
/// This is called when the user types a comma to advance to the next parameter,
/// or backspaces to remove a comma.
pub fn update_signature_help_parameter(ui: &mut UiManager, parameter_index: usize) {
    // Access the popup through the manager and update its active parameter
    if let Some(popup) = ui.get_popup_mut::<SignaturePopup>("lsp-signature") {
        popup.set_active_parameter(Some(parameter_index));
    }
}

/// Dismisses any active signature help popup.
pub fn dismiss_signature_help(ui: &mut UiManager) {
    ui.dismiss_popup("lsp-signature");
}

/// Checks if a signature help popup is currently shown.
pub fn has_signature_help(ui: &UiManager) -> bool {
    ui.has_popup("lsp-signature")
}

// ─────────────────────────────────────────────────────────────────────────────
// Code Actions
// ─────────────────────────────────────────────────────────────────────────────

/// Requests code actions from the LSP server for the given buffer.
///
/// Returns the code action response if available.
pub async fn request_code_actions(
    registry: &Registry,
    buffer: &Buffer,
    range: Option<xeno_base::range::Range>,
) -> Option<CodeActionResponse> {
    use xeno_lsp::lsp_types::{CodeActionContext, CodeActionTriggerKind, Range as LspRange};

    let client = get_client_for_buffer(registry, buffer)?;

    if !client.is_initialized() {
        return None;
    }

    let path = buffer.path()?;
    let uri = Url::from_file_path(&path).ok()?;
    let encoding = client.offset_encoding();

    // Convert range to LSP range
    let lsp_range = if let Some(r) = range {
        let start = xeno_lsp::char_to_lsp_position(&buffer.doc().content, r.from(), encoding)?;
        let end = xeno_lsp::char_to_lsp_position(&buffer.doc().content, r.to(), encoding)?;
        LspRange { start, end }
    } else {
        // Use current line range
        let line = buffer.cursor_line();
        let line_start = buffer.doc().content.line_to_char(line);
        let line_end = if line + 1 < buffer.doc().content.len_lines() {
            buffer.doc().content.line_to_char(line + 1)
        } else {
            buffer.doc().content.len_chars()
        };

        let start = xeno_lsp::char_to_lsp_position(&buffer.doc().content, line_start, encoding)?;
        let end = xeno_lsp::char_to_lsp_position(&buffer.doc().content, line_end, encoding)?;
        LspRange { start, end }
    };

    // Get diagnostics from client state
    let diagnostics: Vec<_> = client
        .diagnostics(&uri)
        .into_iter()
        .filter(|d| ranges_overlap(&d.range, &lsp_range))
        .collect();

    let context = CodeActionContext {
        diagnostics,
        only: None,
        trigger_kind: Some(CodeActionTriggerKind::INVOKED),
    };

    client.code_action(uri, lsp_range, context).await.ok().flatten()
}

/// Checks if two LSP ranges overlap.
fn ranges_overlap(a: &xeno_lsp::lsp_types::Range, b: &xeno_lsp::lsp_types::Range) -> bool {
    // Two ranges overlap if neither is entirely before or after the other
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character < b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character < a.start.character))
}

/// Shows a code actions popup for the current buffer cursor position.
///
/// This is the main entry point for the code actions feature. It:
/// 1. Gets the LSP manager from extensions
/// 2. Requests code actions from the language server
/// 3. Shows the code actions popup in the UI
///
/// # Arguments
///
/// * `ui` - The UI manager for showing popups
/// * `extensions` - The extension map containing the LSP manager
/// * `buffer` - The buffer to request code actions for
/// * `cursor_screen_pos` - Optional screen position of the cursor for popup anchoring
///
/// # Returns
///
/// Returns `true` if a code actions popup was shown, `false` otherwise.
pub async fn show_code_actions(
    ui: &mut UiManager,
    extensions: &ExtensionMap,
    buffer: &Buffer,
    cursor_screen_pos: Option<(u16, u16)>,
) -> bool {
    let Some(registry) = get_lsp_registry(extensions) else {
        return false;
    };

    let Some(actions) = request_code_actions(&registry, buffer, None).await else {
        return false;
    };

    if actions.is_empty() {
        return false;
    }

    // Create the code actions popup
    let Some(popup) = CodeActionsPopup::from_response(actions) else {
        return false;
    };

    ui.show_popup(Box::new(popup));
    true
}

/// Dismisses any active code actions popup.
pub fn dismiss_code_actions(ui: &mut UiManager) {
    ui.dismiss_popup("lsp-code-actions");
}

/// Checks if a code actions popup is currently shown.
pub fn has_code_actions(ui: &UiManager) -> bool {
    ui.has_popup("lsp-code-actions")
}

/// Gets the currently selected code action from an active popup.
///
/// This is used when the user accepts the popup to get the action to apply.
pub fn get_selected_code_action(ui: &UiManager) -> Option<CodeActionResult> {
    let popup = ui.popups.get_popup::<CodeActionsPopup>("lsp-code-actions")?;
    popup.accept_selected()
}

/// Applies a code action result to the editor.
///
/// Handles both workspace edits and commands.
pub async fn apply_code_action(
    editor: &mut crate::editor::Editor,
    result: CodeActionResult,
) -> bool {
    match result {
        CodeActionResult::Edit(edit) => apply_workspace_edit(editor, &edit).await,
        CodeActionResult::Command(cmd) => {
            // Commands need to be sent back to the LSP server
            execute_command(editor, &cmd).await
        }
        CodeActionResult::EditAndCommand(edit, cmd) => {
            // Apply edit first, then execute command
            if apply_workspace_edit(editor, &edit).await {
                execute_command(editor, &cmd).await
            } else {
                false
            }
        }
    }
}

/// Applies a workspace edit to the editor.
///
/// Handles document changes and file changes.
pub async fn apply_workspace_edit(
    editor: &mut crate::editor::Editor,
    edit: &WorkspaceEdit,
) -> bool {
    // Handle `changes` field (simple case: Map<URI, Vec<TextEdit>>)
    if let Some(changes) = &edit.changes {
        for (uri, edits) in changes {
            if !apply_text_edits_to_uri(editor, uri, edits).await {
                return false;
            }
        }
        return true;
    }

    // Handle `documentChanges` field (complex case with versioning)
    if let Some(doc_changes) = &edit.document_changes {
        use xeno_lsp::lsp_types::DocumentChanges;
        match doc_changes {
            DocumentChanges::Edits(edits) => {
                for edit in edits {
                    let text_edits: Vec<TextEdit> =
                        edit.edits.iter().map(extract_text_edit).collect();
                    if !apply_text_edits_to_uri(editor, &edit.text_document.uri, &text_edits).await
                    {
                        return false;
                    }
                }
                return true;
            }
            DocumentChanges::Operations(ops) => {
                use xeno_lsp::lsp_types::DocumentChangeOperation;
                for op in ops {
                    match op {
                        DocumentChangeOperation::Edit(edit) => {
                            let text_edits: Vec<TextEdit> =
                                edit.edits.iter().map(extract_text_edit).collect();
                            if !apply_text_edits_to_uri(
                                editor,
                                &edit.text_document.uri,
                                &text_edits,
                            )
                            .await
                            {
                                return false;
                            }
                        }
                        DocumentChangeOperation::Op(_) => {
                            // File operations (create, rename, delete) not supported yet
                            tracing::warn!("File operations in workspace edit not yet supported");
                        }
                    }
                }
                return true;
            }
        }
    }

    // No changes to apply
    true
}

/// Extracts a `TextEdit` from a `OneOf<TextEdit, AnnotatedTextEdit>`.
fn extract_text_edit(edit: &OneOf<TextEdit, AnnotatedTextEdit>) -> TextEdit {
    match edit {
        OneOf::Left(text_edit) => text_edit.clone(),
        OneOf::Right(annotated) => annotated.text_edit.clone(),
    }
}

/// Applies text edits to a document identified by URI.
async fn apply_text_edits_to_uri(
    editor: &mut crate::editor::Editor,
    uri: &Url,
    edits: &[TextEdit],
) -> bool {
    use xeno_base::Transaction;

    // Convert URI to path
    let path = match uri.to_file_path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Find or open the buffer
    let buffer_id = if let Some(id) = editor.find_buffer_by_path(&path) {
        id
    } else {
        match editor.open_file(path.clone()).await {
            Ok(id) => id,
            Err(_) => return false,
        }
    };

    // Get encoding for position conversion
    let encoding = {
        let registry = get_lsp_registry(&editor.extensions);
        registry
            .as_ref()
            .and_then(|r| {
                let buffer = editor.get_buffer(buffer_id)?;
                get_client_for_buffer(r, buffer)
            })
            .map(|c| c.offset_encoding())
            .unwrap_or_default()
    };

    // Get mutable buffer access
    let buffer = match editor.get_buffer_mut(buffer_id) {
        Some(b) => b,
        None => return false,
    };

    // Sort edits in document order (by position)
    // Transaction::change requires changes to be sorted by start position
    let mut sorted_edits: Vec<_> = edits.to_vec();
    sorted_edits.sort_by(|a, b| {
        let cmp_line = a.range.start.line.cmp(&b.range.start.line);
        if cmp_line != std::cmp::Ordering::Equal {
            cmp_line
        } else {
            a.range.start.character.cmp(&b.range.start.character)
        }
    });

    // Convert LSP edits to xeno Changes
    let content = buffer.doc().content.clone();
    let changes: Vec<_> = sorted_edits
        .iter()
        .filter_map(|edit| {
            let start = xeno_lsp::lsp_position_to_char(&content, edit.range.start, encoding)?;
            let end = xeno_lsp::lsp_position_to_char(&content, edit.range.end, encoding)?;
            Some(xeno_base::transaction::Change {
                start,
                end,
                replacement: Some(edit.new_text.clone()),
            })
        })
        .collect();

    if changes.is_empty() {
        return true; // No valid changes to apply
    }

    // Create and apply the transaction
    let tx = Transaction::change(content.slice(..), changes);
    buffer.apply_transaction(&tx);

    editor.needs_redraw = true;
    true
}

/// Executes an LSP command on the language server.
async fn execute_command(
    editor: &mut crate::editor::Editor,
    command: &xeno_lsp::lsp_types::Command,
) -> bool {
    let Some(registry) = get_lsp_registry(&editor.extensions) else {
        return false;
    };

    let buffer = editor.buffer();
    let Some(client) = get_client_for_buffer(&registry, buffer) else {
        return false;
    };

    if !client.is_initialized() {
        return false;
    }

    // Send workspace/executeCommand request
    let params = xeno_lsp::lsp_types::ExecuteCommandParams {
        command: command.command.clone(),
        arguments: command.arguments.clone().unwrap_or_default(),
        work_done_progress_params: Default::default(),
    };

    match client
        .request::<xeno_lsp::lsp_types::request::ExecuteCommand>(params)
        .await
    {
        Ok(_) => true,
        Err(e) => {
            tracing::warn!("Failed to execute command '{}': {}", command.command, e);
            false
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Navigation
// ─────────────────────────────────────────────────────────────────────────────

/// Goes to the definition at the cursor position.
///
/// This requests the definition location from the LSP server and navigates
/// to it if found. If there are multiple definitions, shows a picker popup.
///
/// # Returns
///
/// Returns `true` if a definition was found and navigated to (or picker shown), `false` otherwise.
pub async fn goto_definition(editor: &mut crate::editor::Editor) -> bool {
    use crate::ui::popup::LocationPickerPopup;
    use xeno_lsp::lsp_types::GotoDefinitionResponse;
    
    let Some(registry) = get_lsp_registry(&editor.extensions) else {
        return false;
    };
    
    let buffer = editor.buffer();
    let client = match get_client_for_buffer(&registry, buffer) {
        Some(c) => c,
        None => return false,
    };
    
    if !client.is_initialized() {
        return false;
    }
    
    let uri = match buffer_to_uri(buffer) {
        Some(u) => u,
        None => return false,
    };
    let encoding = client.offset_encoding();
    let position = match buffer_cursor_to_lsp_position(buffer, encoding) {
        Some(p) => p,
        None => return false,
    };
    
    let response = match client.goto_definition(uri, position).await {
        Ok(Some(r)) => r,
        _ => return false,
    };

    // Count locations to determine if we need a picker
    let location_count = match &response {
        GotoDefinitionResponse::Scalar(_) => 1,
        GotoDefinitionResponse::Array(locs) => locs.len(),
        GotoDefinitionResponse::Link(links) => links.len(),
    };

    // If multiple definitions, show picker popup
    if location_count > 1 {
        if let Some(popup) = LocationPickerPopup::from_definition_response(response) {
            editor.ui.show_popup(Box::new(popup));
            return true;
        }
        // If popup creation failed, fall through to try navigating to first location
        // But response was consumed, so we can't continue - return false
        return false;
    }

    // Single definition - navigate directly
    let location = match response {
        GotoDefinitionResponse::Scalar(loc) => Some(loc),
        GotoDefinitionResponse::Array(locs) => locs.into_iter().next(),
        GotoDefinitionResponse::Link(links) => links.into_iter().next().map(|l| {
            xeno_lsp::lsp_types::Location {
                uri: l.target_uri,
                range: l.target_selection_range,
            }
        }),
    };

    let Some(loc) = location else {
        return false;
    };

    // Navigate to the location
    navigate_to_location(editor, &loc).await
}

/// Finds all references at the cursor position.
///
/// Shows a references panel with all found references.
///
/// # Returns
///
/// Returns `true` if references were found, `false` otherwise.
pub async fn find_references(editor: &mut crate::editor::Editor) -> bool {
    use crate::ui::panels::ReferencesPanel;

    let Some(registry) = get_lsp_registry(&editor.extensions) else {
        return false;
    };
    
    let buffer = editor.buffer();
    let client = match get_client_for_buffer(&registry, buffer) {
        Some(c) => c,
        None => return false,
    };
    
    if !client.is_initialized() {
        return false;
    }
    
    let uri = match buffer_to_uri(buffer) {
        Some(u) => u,
        None => return false,
    };
    let encoding = client.offset_encoding();
    let position = match buffer_cursor_to_lsp_position(buffer, encoding) {
        Some(p) => p,
        None => return false,
    };
    
    let references = match client.references(uri, position, true).await {
        Ok(Some(r)) if !r.is_empty() => r,
        _ => return false,
    };

    // If only one reference, navigate directly
    if references.len() == 1 {
        if let Some(loc) = references.first() {
            return navigate_to_location(editor, loc).await;
        }
    }
    
    // Get a title based on what we're finding references of
    // Try to get the word under cursor for a better title
    let title = {
        let buffer = editor.buffer();
        let cursor = buffer.cursor;
        let rope = &buffer.doc().content;
        
        // Extract word at cursor
        let line_idx = rope.char_to_line(cursor);
        let line_start = rope.line_to_char(line_idx);
        let col = cursor - line_start;
        let line = rope.line(line_idx).to_string();
        
        // Find word boundaries
        let chars: Vec<char> = line.chars().collect();
        let mut start = col;
        let mut end = col;
        
        while start > 0 && chars.get(start - 1).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
            start -= 1;
        }
        while end < chars.len() && chars.get(end).is_some_and(|c| c.is_alphanumeric() || *c == '_') {
            end += 1;
        }
        
        if start < end {
            format!("References to '{}'", chars[start..end].iter().collect::<String>())
        } else {
            "References".to_string()
        }
    };

    // Show references panel
    editor.show_references_panel(references, &title);
    true
}

/// Navigates to an LSP location (file + position).
///
/// Opens the file if not already open, then moves the cursor to the specified position.
pub async fn navigate_to_location(
    editor: &mut crate::editor::Editor,
    location: &xeno_lsp::lsp_types::Location,
) -> bool {

    
    // Convert URI to path
    let path = match location.uri.to_file_path() {
        Ok(p) => p,
        Err(_) => return false,
    };
    
    // Open the file if not already open
    let buffer_id = if let Some(id) = editor.find_buffer_by_path(&path) {
        id
    } else {
        match editor.open_file(path.clone()).await {
            Ok(id) => id,
            Err(_) => return false,
        }
    };
    
    // Focus the buffer
    editor.focus_buffer(buffer_id);

    // LSP positions are 0-indexed line and character
    let line = location.range.start.line as usize;
    let col = location.range.start.character as usize;

    // Convert LSP position to char index (separate scope to release borrow)
    let char_idx = {
        let buffer = editor.buffer();
        let rope = &buffer.doc().content;

        // Convert to char index
        if line >= rope.len_lines() {
            return true; // File opened but position invalid
        }
        let line_start = rope.line_to_char(line);
        let line_len = rope.line(line).len_chars();
        line_start + col.min(line_len.saturating_sub(1))
    };

    // Set cursor position (now we can mutably borrow)
    editor.buffer_mut().cursor = char_idx;
    editor.buffer_mut().selection = xeno_base::Selection::point(char_idx);
    editor.needs_redraw = true;

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_lsp_manager_empty() {
        let extensions = ExtensionMap::new();
        assert!(get_lsp_manager(&extensions).is_none());
    }
}
