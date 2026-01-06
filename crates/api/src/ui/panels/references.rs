//! References panel for displaying LSP find references results.
//!
//! Provides a persistent list of all references with navigation support.

use std::path::PathBuf;


use termina::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use xeno_lsp::lsp_types::{Location, Url};
use xeno_registry::themes::Theme;
use xeno_tui::layout::{Position, Rect};
use xeno_tui::style::{Modifier, Style};
use xeno_tui::text::{Line, Span};

use xeno_tui::Frame;


use crate::editor::Editor;
use crate::ui::dock::DockSlot;
use crate::ui::panel::{CursorRequest, EventResult, Panel, UiEvent, UiRequest};

/// A single reference entry in the panel.
#[derive(Debug, Clone)]
pub struct ReferenceEntry {
    /// File path where the reference occurs.
    pub path: PathBuf,
    /// 0-indexed line number.
    pub line: usize,
    /// 0-indexed column number.
    pub col: usize,
    /// Optional context/preview text from the line.
    pub context: Option<String>,
    /// The original URI.
    pub uri: Url,
}

impl ReferenceEntry {
    /// Creates a reference entry from an LSP location.
    pub fn from_location(location: &Location) -> Option<Self> {
        let path = location.uri.to_file_path().ok()?;
        let line = location.range.start.line as usize;
        let col = location.range.start.character as usize;

        Some(Self {
            path,
            line,
            col,
            context: None,
            uri: location.uri.clone(),
        })
    }

    /// Creates a reference entry with context text.
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }
}

/// Panel displaying LSP find references results with navigation.
pub struct ReferencesPanel {
    /// All reference entries.
    references: Vec<ReferenceEntry>,
    /// Currently selected index.
    selected: usize,
    /// Scroll offset for display.
    scroll_offset: usize,
    /// Title/label for what we're finding references of.
    title: String,
    /// Cached visible height from last render.
    visible_height: usize,
}

impl Default for ReferencesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferencesPanel {
    /// Panel identifier.
    pub const ID: &'static str = "references";

    /// Creates a new empty references panel.
    pub fn new() -> Self {
        Self {
            references: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            title: "References".to_string(),
            visible_height: 10,
        }
    }

    /// Sets the references to display.
    pub fn set_references(&mut self, locations: Vec<Location>, title: &str) {
        self.references.clear();
        for loc in &locations {
            if let Some(entry) = ReferenceEntry::from_location(loc) {
                self.references.push(entry);
            }
        }

        // Sort by file, then by line
        self.references.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then_with(|| a.line.cmp(&b.line))
                .then_with(|| a.col.cmp(&b.col))
        });

        self.title = title.to_string();
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Loads context snippets for all references from open buffers.
    pub fn load_contexts(&mut self, editor: &Editor) {
        for entry in &mut self.references {
            // Try to get context from an open buffer
            if let Some(buffer_id) = editor.find_buffer_by_path(&entry.path) {
                if let Some(buffer) = editor.get_buffer(buffer_id) {
                    let rope = &buffer.doc().content;
                    if entry.line < rope.len_lines() {
                        let line_text = rope.line(entry.line).to_string();
                        // Trim and truncate the context
                        let trimmed = line_text.trim();
                        let truncated = if trimmed.len() > 60 {
                            format!("{}...", &trimmed[..57])
                        } else {
                            trimmed.to_string()
                        };
                        entry.context = Some(truncated);
                    }
                }
            }
        }
    }

    /// Returns the number of references.
    pub fn len(&self) -> usize {
        self.references.len()
    }

    /// Returns true if there are no references.
    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    /// Returns the currently selected reference entry.
    pub fn selected_entry(&self) -> Option<&ReferenceEntry> {
        self.references.get(self.selected)
    }

    /// Selects the next reference.
    pub fn select_next(&mut self) {
        if !self.references.is_empty() {
            self.selected = (self.selected + 1) % self.references.len();
            self.ensure_visible();
        }
    }

    /// Selects the previous reference.
    pub fn select_prev(&mut self) {
        if !self.references.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.references.len() - 1);
            self.ensure_visible();
        }
    }

    /// Ensures the selected item is visible in the viewport.
    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.visible_height {
            self.scroll_offset = self.selected.saturating_sub(self.visible_height - 1);
        }
    }

    /// Handles key events.
    fn handle_key(&mut self, key: KeyEvent, editor: &mut Editor) -> EventResult {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                EventResult::consumed()
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_prev();
                EventResult::consumed()
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                // Jump to selected reference
                if let Some(entry) = self.selected_entry() {
                    self.jump_to_reference(editor, entry.clone());
                }
                EventResult::consumed()
            }
            KeyCode::Escape | KeyCode::Char('q') => {
                // Close panel
                EventResult::consumed().with_request(UiRequest::ClosePanel(Self::ID.to_string()))
            }
            KeyCode::Home | KeyCode::Char('g') => {
                // Jump to first reference
                self.selected = 0;
                self.ensure_visible();
                EventResult::consumed()
            }
            KeyCode::End | KeyCode::Char('G') => {
                // Jump to last reference
                if !self.references.is_empty() {
                    self.selected = self.references.len() - 1;
                    self.ensure_visible();
                }
                EventResult::consumed()
            }
            _ => EventResult::not_consumed(),
        }
    }

    /// Handles mouse events.
    fn handle_mouse(
        &mut self,
        mouse: MouseEvent,
        area: Rect,
        editor: &mut Editor,
    ) -> EventResult {
        // Check if mouse is in our area
        if mouse.row < area.y || mouse.row >= area.y + area.height {
            return EventResult::not_consumed();
        }
        if mouse.column < area.x || mouse.column >= area.x + area.width {
            return EventResult::not_consumed();
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Calculate which row was clicked (accounting for header)
                let clicked_row = (mouse.row - area.y) as usize;
                if clicked_row >= 2 {
                    // Skip header (2 lines)
                    let ref_idx = self.scroll_offset + clicked_row - 2;
                    if ref_idx < self.references.len() {
                        self.selected = ref_idx;
                        return EventResult::consumed();
                    }
                }
                EventResult::consumed()
            }
            MouseEventKind::ScrollUp => {
                self.select_prev();
                EventResult::consumed()
            }
            MouseEventKind::ScrollDown => {
                self.select_next();
                EventResult::consumed()
            }
            MouseEventKind::Up(MouseButton::Left) => EventResult::consumed(),
            _ => EventResult::not_consumed(),
        }
    }

    /// Jumps to a reference location in the editor.
    fn jump_to_reference(&self, editor: &mut Editor, entry: ReferenceEntry) {
        // Find or open the buffer
        let buffer_id = if let Some(id) = editor.find_buffer_by_path(&entry.path) {
            id
        } else {
            // We can't async open here, so just return
            // The user can open the file manually
            return;
        };

        // Focus the buffer
        editor.focus_buffer(buffer_id);

        // Navigate to position - compute char_idx in separate scope to avoid borrow conflicts
        let char_idx = {
            let buffer = editor.buffer();
            let rope = &buffer.doc().content;
            if entry.line < rope.len_lines() {
                let line_start = rope.line_to_char(entry.line);
                let line_len = rope.line(entry.line).len_chars();
                Some(line_start + entry.col.min(line_len.saturating_sub(1)))
            } else {
                None
            }
        };

        if let Some(char_idx) = char_idx {
            let buffer = editor.buffer_mut();
            buffer.cursor = char_idx;
            buffer.selection = xeno_base::Selection::point(char_idx);
        }

        editor.needs_redraw = true;
    }

    /// Renders the panel header.
    fn render_header(&self, frame: &mut Frame<'_>, area: Rect, theme: &Theme) {
        // Title line
        let title = Line::from(vec![
            Span::styled(
                format!(" {} ", self.title),
                Style::default()
                    .fg(theme.colors.ui.fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("({} results)", self.references.len()),
                Style::default().fg(theme.colors.status.dim_fg),
            ),
        ]);

        // Render title
        if area.height > 0 {
            frame.render_widget(
                xeno_tui::widgets::Paragraph::new(title),
                Rect::new(area.x, area.y, area.width, 1),
            );
        }

        // Render separator
        if area.height > 1 {
            let sep = "─".repeat(area.width as usize);
            frame.render_widget(
                xeno_tui::widgets::Paragraph::new(Line::styled(
                    sep,
                    Style::default().fg(theme.colors.popup.border),
                )),
                Rect::new(area.x, area.y + 1, area.width, 1),
            );
        }
    }

    /// Renders a single reference line.
    fn render_reference_line(
        &self,
        entry: &ReferenceEntry,
        selected: bool,
        width: u16,
        theme: &Theme,
    ) -> Line<'static> {
        // Format: filename:line:col  context
        let filename = entry
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        let location = format!("{}:{}:{}", filename, entry.line + 1, entry.col + 1);

        // Calculate available space for context
        let prefix_len = location.len() + 2;
        let max_context_len = (width as usize).saturating_sub(prefix_len + 3);

        let context = entry
            .context
            .as_ref()
            .map(|c| {
                if c.len() > max_context_len {
                    format!("{}...", &c[..max_context_len.saturating_sub(3)])
                } else {
                    c.clone()
                }
            })
            .unwrap_or_default();

        let bg = if selected {
            theme.colors.popup.selection
        } else {
            theme.colors.ui.bg
        };

        Line::from(vec![
            Span::styled(
                format!(" {:<20}", location),
                Style::default().fg(theme.colors.status.accent_fg).bg(bg),
            ),
            Span::styled(context, Style::default().fg(theme.colors.status.dim_fg).bg(bg)),
        ])
    }
}

impl Panel for ReferencesPanel {
    fn id(&self) -> &str {
        Self::ID
    }

    fn default_slot(&self) -> DockSlot {
        DockSlot::Bottom
    }

    fn handle_event(&mut self, event: UiEvent, editor: &mut Editor, focused: bool) -> EventResult {
        match event {
            UiEvent::Key(key) if focused => self.handle_key(key, editor),
            UiEvent::Mouse(mouse) => {
                // Mouse handling needs area, which we don't have here
                EventResult::not_consumed()
            }
            _ => EventResult::not_consumed(),
        }
    }

    fn render(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        editor: &mut Editor,
        focused: bool,
        theme: &Theme,
    ) -> Option<CursorRequest> {
        // Update visible height for scroll calculations
        self.visible_height = area.height.saturating_sub(2) as usize;

        // Load contexts if not already loaded
        if self.references.iter().any(|r| r.context.is_none()) {
            self.load_contexts(editor);
        }

        // Draw background
        let bg = theme.colors.ui.bg;
        let block = xeno_tui::widgets::Block::default().style(Style::default().bg(bg));
        frame.render_widget(block, area);

        // Render header
        self.render_header(frame, area, theme);

        // Render references list
        let list_area = Rect::new(area.x, area.y + 2, area.width, area.height.saturating_sub(2));

        if self.references.is_empty() {
            let msg = Line::styled(
                " No references found",
                Style::default().fg(theme.colors.status.dim_fg),
            );
            frame.render_widget(
                xeno_tui::widgets::Paragraph::new(msg),
                Rect::new(list_area.x, list_area.y, list_area.width, 1),
            );
        } else {
            // Render visible references
            let end = (self.scroll_offset + self.visible_height).min(self.references.len());
            for (i, entry) in self.references[self.scroll_offset..end].iter().enumerate() {
                let is_selected = self.scroll_offset + i == self.selected;
                let line = self.render_reference_line(entry, is_selected, area.width, theme);
                frame.render_widget(
                    xeno_tui::widgets::Paragraph::new(line),
                    Rect::new(list_area.x, list_area.y + i as u16, list_area.width, 1),
                );
            }
        }

        // Return cursor position for selected item if focused
        if focused && !self.references.is_empty() {
            let cursor_y = area.y + 2 + (self.selected - self.scroll_offset) as u16;
            Some(CursorRequest {
                pos: Position::new(area.x, cursor_y),
                style: None,
            })
        } else {
            None
        }
    }
}
