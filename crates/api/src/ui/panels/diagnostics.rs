//! Diagnostics panel for displaying LSP diagnostic messages.
//!
//! Provides a persistent list of all diagnostics with navigation support.

use std::path::PathBuf;
use std::sync::Arc;

use termina::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use xeno_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Url};
use xeno_registry::themes::Theme;
use xeno_tui::layout::{Position, Rect};
use xeno_tui::style::{Color, Modifier, Style};
use xeno_tui::text::{Line, Span};

use xeno_tui::Frame;

use crate::editor::Editor;
use crate::editor::extensions::ExtensionMap;
use crate::lsp::LspManager;
use crate::ui::dock::DockSlot;
use crate::ui::panel::{CursorRequest, EventResult, Panel, UiEvent, UiRequest};

/// A single diagnostic entry in the panel.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    /// File path where the diagnostic occurred.
    pub path: PathBuf,
    /// 0-indexed line number.
    pub line: usize,
    /// 0-indexed column number.
    pub col: usize,
    /// Severity level (1=error, 2=warning, 3=info, 4=hint).
    pub severity: u8,
    /// Diagnostic message.
    pub message: String,
    /// Source of the diagnostic (e.g., "rustc", "clippy").
    pub source: Option<String>,
}

impl DiagnosticEntry {
    /// Creates a new diagnostic entry from an LSP diagnostic.
    pub fn from_lsp(uri: &Url, diagnostic: &Diagnostic) -> Option<Self> {
        let path = uri.to_file_path().ok()?;
        let line = diagnostic.range.start.line as usize;
        let col = diagnostic.range.start.character as usize;
        let severity = match diagnostic.severity {
            Some(DiagnosticSeverity::ERROR) => 1,
            Some(DiagnosticSeverity::WARNING) => 2,
            Some(DiagnosticSeverity::INFORMATION) => 3,
            Some(DiagnosticSeverity::HINT) => 4,
            _ => 1,
        };

        Some(Self {
            path,
            line,
            col,
            severity,
            message: diagnostic.message.clone(),
            source: diagnostic.source.clone(),
        })
    }

    /// Returns the severity as a string label.
    pub fn severity_label(&self) -> &'static str {
        match self.severity {
            1 => "error",
            2 => "warn",
            3 => "info",
            4 => "hint",
            _ => "???",
        }
    }

    /// Returns the severity color.
    pub fn severity_color(&self, theme: &Theme) -> Color {
        match self.severity {
            1 => theme.colors.diagnostic.error,
            2 => theme.colors.diagnostic.warning,
            3 => theme.colors.diagnostic.info,
            4 => theme.colors.diagnostic.hint,
            _ => theme.colors.ui.fg,
        }
    }
}

/// Filter for diagnostics display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticFilter {
    /// Show all diagnostics.
    #[default]
    All,
    /// Show only errors.
    Errors,
    /// Show errors and warnings.
    ErrorsAndWarnings,
    /// Show only diagnostics from the current buffer.
    CurrentBuffer,
}

impl DiagnosticFilter {
    /// Returns the next filter in the cycle.
    pub fn cycle(self) -> Self {
        match self {
            Self::All => Self::Errors,
            Self::Errors => Self::ErrorsAndWarnings,
            Self::ErrorsAndWarnings => Self::CurrentBuffer,
            Self::CurrentBuffer => Self::All,
        }
    }

    /// Returns a label for this filter.
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Errors => "Errors",
            Self::ErrorsAndWarnings => "Errors+Warnings",
            Self::CurrentBuffer => "Current File",
        }
    }

    /// Checks if a diagnostic passes this filter.
    pub fn matches(&self, entry: &DiagnosticEntry, current_path: Option<&PathBuf>) -> bool {
        match self {
            Self::All => true,
            Self::Errors => entry.severity == 1,
            Self::ErrorsAndWarnings => entry.severity <= 2,
            Self::CurrentBuffer => current_path.is_some_and(|p| &entry.path == p),
        }
    }
}

/// Panel displaying LSP diagnostics with navigation.
pub struct DiagnosticsPanel {
    /// All diagnostic entries.
    diagnostics: Vec<DiagnosticEntry>,
    /// Currently selected index.
    selected: usize,
    /// Scroll offset for display.
    scroll_offset: usize,
    /// Current filter.
    filter: DiagnosticFilter,
    /// Last seen diagnostic revision for change detection.
    last_revision: u64,
    /// Cached visible height from last render.
    visible_height: usize,
}

impl Default for DiagnosticsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticsPanel {
    /// Panel identifier.
    pub const ID: &'static str = "diagnostics";

    /// Creates a new diagnostics panel.
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            filter: DiagnosticFilter::default(),
            last_revision: 0,
            visible_height: 10,
        }
    }

    /// Updates diagnostics from the LSP manager.
    pub fn refresh(&mut self, extensions: &ExtensionMap, current_path: Option<&PathBuf>) {
        let Some(lsp) = extensions.get::<Arc<LspManager>>() else {
            self.diagnostics.clear();
            return;
        };

        let revision = lsp.diagnostic_revision();
        if revision == self.last_revision && !self.diagnostics.is_empty() {
            // No changes, just refilter if needed
            return;
        }
        self.last_revision = revision;

        // Collect all diagnostics
        let all_diagnostics = lsp.all_diagnostics();
        self.diagnostics.clear();

        for (uri, diags) in all_diagnostics {
            for diag in diags {
                if let Some(entry) = DiagnosticEntry::from_lsp(&uri, &diag) {
                    if self.filter.matches(&entry, current_path) {
                        self.diagnostics.push(entry);
                    }
                }
            }
        }

        // Sort by severity (errors first), then by file, then by line
        self.diagnostics.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.line.cmp(&b.line))
        });

        // Clamp selection
        if self.selected >= self.diagnostics.len() {
            self.selected = self.diagnostics.len().saturating_sub(1);
        }

        self.ensure_visible();
    }

    /// Returns the number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns true if there are no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Returns the currently selected diagnostic entry.
    pub fn selected_entry(&self) -> Option<&DiagnosticEntry> {
        self.diagnostics.get(self.selected)
    }

    /// Selects the next diagnostic.
    pub fn select_next(&mut self) {
        if !self.diagnostics.is_empty() {
            self.selected = (self.selected + 1) % self.diagnostics.len();
            self.ensure_visible();
        }
    }

    /// Selects the previous diagnostic.
    pub fn select_prev(&mut self) {
        if !self.diagnostics.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.diagnostics.len() - 1);
            self.ensure_visible();
        }
    }

    /// Cycles through filter modes.
    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.cycle();
        // Force refresh on next tick
        self.last_revision = 0;
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
            KeyCode::Char('j') | KeyCode::Char('n') | KeyCode::Down => {
                self.select_next();
                EventResult::consumed()
            }
            KeyCode::Char('k') | KeyCode::Char('N') | KeyCode::Up => {
                self.select_prev();
                EventResult::consumed()
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                // Jump to selected diagnostic
                if let Some(entry) = self.selected_entry() {
                    self.jump_to_diagnostic(editor, entry.clone());
                }
                EventResult::consumed()
            }
            KeyCode::Char('f') => {
                // Cycle filter
                self.cycle_filter();
                EventResult::consumed()
            }
            KeyCode::Escape | KeyCode::Char('q') => {
                // Close panel
                EventResult::consumed().with_request(UiRequest::ClosePanel(Self::ID.to_string()))
            }
            KeyCode::Home | KeyCode::Char('g') => {
                // Jump to first diagnostic
                self.selected = 0;
                self.ensure_visible();
                EventResult::consumed()
            }
            KeyCode::End | KeyCode::Char('G') => {
                // Jump to last diagnostic
                if !self.diagnostics.is_empty() {
                    self.selected = self.diagnostics.len() - 1;
                    self.ensure_visible();
                }
                EventResult::consumed()
            }
            _ => EventResult::not_consumed(),
        }
    }

    /// Handles mouse events.
    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect, editor: &mut Editor) -> EventResult {
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
                    let diag_idx = self.scroll_offset + clicked_row - 2;
                    if diag_idx < self.diagnostics.len() {
                        self.selected = diag_idx;
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
            MouseEventKind::Up(MouseButton::Left) => {
                // Double-click behavior: jump to diagnostic
                // For simplicity, we just consume here
                EventResult::consumed()
            }
            _ => EventResult::not_consumed(),
        }
    }

    /// Jumps to a diagnostic location in the editor.
    fn jump_to_diagnostic(&self, editor: &mut Editor, entry: DiagnosticEntry) {
        use crate::buffer::BufferId;

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
        let error_count = self.diagnostics.iter().filter(|d| d.severity == 1).count();
        let warning_count = self.diagnostics.iter().filter(|d| d.severity == 2).count();

        // Title line
        let title = Line::from(vec![
            Span::styled(
                " Diagnostics ",
                Style::default()
                    .fg(theme.colors.ui.fg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("[{}] ", self.filter.label()),
                Style::default().fg(theme.colors.status.dim_fg),
            ),
            Span::styled(
                format!("{} ", error_count),
                Style::default().fg(theme.colors.diagnostic.error),
            ),
            Span::styled(
                format!("{}", warning_count),
                Style::default().fg(theme.colors.diagnostic.warning),
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

    /// Renders a single diagnostic line.
    fn render_diagnostic_line(
        &self,
        entry: &DiagnosticEntry,
        selected: bool,
        width: u16,
        theme: &Theme,
    ) -> Line<'static> {
        let sev_color = entry.severity_color(theme);

        // Format: filename:line:col  severity  message
        let filename = entry
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        let location = format!("{}:{}:{}", filename, entry.line + 1, entry.col + 1);

        // Severity icon
        let sev_icon = match entry.severity {
            1 => "E",
            2 => "W",
            3 => "I",
            4 => "H",
            _ => "?",
        };

        // Calculate available space for message
        let prefix_len = location.len() + 4 + 2; // location + " E " + padding
        let max_msg_len = (width as usize).saturating_sub(prefix_len);
        let msg = if entry.message.len() > max_msg_len {
            format!("{}...", &entry.message[..max_msg_len.saturating_sub(3)])
        } else {
            entry.message.clone()
        };

        let bg = if selected {
            theme.colors.popup.selection
        } else {
            theme.colors.ui.bg
        };

        Line::from(vec![
            Span::styled(
                format!("{:<20}", location),
                Style::default().fg(theme.colors.status.dim_fg).bg(bg),
            ),
            Span::styled(
                format!(" {} ", sev_icon),
                Style::default().fg(sev_color).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled(msg, Style::default().fg(theme.colors.ui.fg).bg(bg)),
        ])
    }
}

impl Panel for DiagnosticsPanel {
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
                // We need area for mouse handling, but we don't have it here
                // Mouse handling will be done in render
                EventResult::not_consumed()
            }
            UiEvent::Tick => {
                // Refresh diagnostics periodically
                let current_path = editor.buffer().path();
                self.refresh(&editor.extensions, current_path.as_ref());
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

        // Refresh diagnostics
        let current_path = editor.buffer().path();
        self.refresh(&editor.extensions, current_path.as_ref());

        // Draw background
        let bg = if focused {
            theme.colors.ui.bg
        } else {
            theme.colors.ui.bg
        };
        let block = xeno_tui::widgets::Block::default().style(Style::default().bg(bg));
        frame.render_widget(block, area);

        // Render header
        self.render_header(frame, area, theme);

        // Render diagnostics list
        let list_area = Rect::new(area.x, area.y + 2, area.width, area.height.saturating_sub(2));

        if self.diagnostics.is_empty() {
            let msg = Line::styled(
                " No diagnostics",
                Style::default().fg(theme.colors.status.dim_fg),
            );
            frame.render_widget(
                xeno_tui::widgets::Paragraph::new(msg),
                Rect::new(list_area.x, list_area.y, list_area.width, 1),
            );
        } else {
            // Render visible diagnostics
            let end = (self.scroll_offset + self.visible_height).min(self.diagnostics.len());
            for (i, entry) in self.diagnostics[self.scroll_offset..end].iter().enumerate() {
                let is_selected = self.scroll_offset + i == self.selected;
                let line = self.render_diagnostic_line(entry, is_selected, area.width, theme);
                frame.render_widget(
                    xeno_tui::widgets::Paragraph::new(line),
                    Rect::new(list_area.x, list_area.y + i as u16, list_area.width, 1),
                );
            }
        }

        // Return cursor position for selected item if focused
        if focused && !self.diagnostics.is_empty() {
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
