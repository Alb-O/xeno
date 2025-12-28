mod actions;
mod actions_exec;
pub mod extensions;
mod history;
mod input_handling;
mod messaging;
mod navigation;
mod search;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use agentfs_sdk::{FileSystem, HostFS};
use tome_base::range::CharIdx;
use tome_base::{Rope, Selection, Transaction};
use tome_input::InputHandler;
use tome_language::LanguageLoader;
use tome_language::syntax::Syntax;
use tome_manifest::syntax::SyntaxStyles;
use tome_manifest::{HookContext, Mode, emit_hook};
use tome_stdlib::movement;
use tome_theme::Theme;

use crate::render::{Notifications, Overflow};
pub use types::{HistoryEntry, Message, MessageKind, Registers};

use crate::editor::extensions::{EXTENSIONS, ExtensionMap, StyleOverlays};
use crate::editor::types::CompletionState;
use crate::ui::UiManager;

pub struct Editor {
	pub doc: Rope,
	pub cursor: CharIdx,
	pub selection: Selection,
	pub input: InputHandler,
	pub path: Option<PathBuf>,
	pub modified: bool,
	pub scroll_line: usize,
	pub scroll_segment: usize,
	pub message: Option<Message>,
	pub registers: Registers,
	pub undo_stack: Vec<HistoryEntry>,
	pub redo_stack: Vec<HistoryEntry>,
	pub text_width: usize,

	pub file_type: Option<String>,
	pub theme: &'static Theme,
	pub window_width: Option<u16>,
	pub window_height: Option<u16>,
	pub ui: UiManager,
	pub needs_redraw: bool,
	pub(crate) insert_undo_active: bool,
	pub notifications: Notifications,
	pub last_tick: std::time::SystemTime,
	#[allow(
		dead_code,
		reason = "IPC server currently only used for internal messaging, but field is read via debug tools"
	)]
	pub ipc: Option<crate::ipc::IpcServer>,
	pub completions: CompletionState,
	pub extensions: ExtensionMap,
	pub fs: Arc<dyn FileSystem>,
	pub language_loader: LanguageLoader,
	pub syntax: Option<Syntax>,
	/// Style overlays for rendering modifications.
	///
	/// Extensions can add overlays during their tick to modify how text is rendered.
	/// Overlays are cleared at the start of each tick cycle.
	pub style_overlays: StyleOverlays,
}

// TextAccess is already implemented in capabilities.rs
// MessageAccess is already implemented in capabilities.rs

impl tome_manifest::editor_ctx::FileOpsAccess for Editor {
	fn is_modified(&self) -> bool {
		self.modified
	}

	fn save(
		&mut self,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), tome_manifest::CommandError>> + '_>,
	> {
		Box::pin(async move {
			let path_owned = match &self.path {
				Some(p) => p.clone(),
				None => {
					return Err(tome_manifest::CommandError::InvalidArgument(
						"No filename. Use :write <filename>".to_string(),
					));
				}
			};

			emit_hook(&HookContext::BufferWritePre {
				path: &path_owned,
				text: self.doc.slice(..),
			});

			let mut content = Vec::new();
			for chunk in self.doc.chunks() {
				content.extend_from_slice(chunk.as_bytes());
			}

			let virtual_path = self.path_to_virtual(&path_owned).ok_or_else(|| {
				tome_manifest::CommandError::Io(format!(
					"Path contains invalid UTF-8: {}",
					path_owned.display()
				))
			})?;
			self.fs
				.write_file(&virtual_path, &content)
				.await
				.map_err(|e| tome_manifest::CommandError::Io(e.to_string()))?;

			self.modified = false;
			self.notify("info", format!("Saved {}", path_owned.display()));

			emit_hook(&HookContext::BufferWrite { path: &path_owned });

			Ok(())
		})
	}

	fn save_as(
		&mut self,
		path: PathBuf,
	) -> std::pin::Pin<
		Box<dyn std::future::Future<Output = Result<(), tome_manifest::CommandError>> + '_>,
	> {
		self.path = Some(path);
		self.save()
	}
}

impl tome_manifest::EditorOps for Editor {}

impl Editor {
	pub async fn new(path: PathBuf) -> anyhow::Result<Self> {
		let cwd = std::env::current_dir()?;
		let fs = Arc::new(HostFS::new(cwd.clone())?);

		// Convert path to virtual path for HostFS
		let virtual_path = Self::compute_virtual_path(&path, &cwd)
			.ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {}", path.display()))?;

		let content = if fs.stat(&virtual_path).await?.is_some() {
			let bytes = fs.read_file(&virtual_path).await?.unwrap_or_default();
			String::from_utf8_lossy(&bytes).to_string()
		} else {
			String::new()
		};

		Ok(Self::from_content(fs, content, Some(path)))
	}

	/// Computes virtual path for HostFS (static version for use before Editor is constructed).
	fn compute_virtual_path(path: &PathBuf, cwd: &PathBuf) -> Option<String> {
		let path_str = path.to_str()?;

		if path.is_absolute() {
			if let Ok(relative) = path.strip_prefix(cwd) {
				return relative.to_str().map(String::from);
			}
		}

		Some(path_str.to_string())
	}

	pub fn new_scratch() -> Self {
		let fs = Arc::new(HostFS::new(std::env::current_dir().unwrap()).unwrap());
		Self::from_content(fs, String::new(), None)
	}

	pub fn from_content(fs: Arc<dyn FileSystem>, content: String, path: Option<PathBuf>) -> Self {
		let doc = Rope::from(content.as_str());

		// Initialize language loader and detect file type
		let mut language_loader = LanguageLoader::new();
		for lang in tome_manifest::LANGUAGES.iter() {
			language_loader.register(lang.into());
		}

		let (file_type, syntax) = if let Some(ref p) = path {
			if let Some(lang_id) = language_loader.language_for_path(p) {
				let lang_data = language_loader.get(lang_id);
				let file_type = lang_data.map(|l| l.name.clone());
				let syntax = Syntax::new(doc.slice(..), lang_id, &language_loader).ok();
				(file_type, syntax)
			} else {
				(None, None)
			}
		} else {
			(None, None)
		};

		let scratch_path = PathBuf::from("[scratch]");
		let hook_path = path.as_ref().unwrap_or(&scratch_path);

		emit_hook(&HookContext::BufferOpen {
			path: hook_path,
			text: doc.slice(..),
			file_type: file_type.as_deref(),
		});

		Self {
			doc,
			cursor: 0,
			selection: Selection::point(0),
			input: InputHandler::new(),
			path,
			modified: false,
			scroll_line: 0,
			scroll_segment: 0,
			message: None,
			registers: Registers::default(),
			undo_stack: Vec::new(),
			redo_stack: Vec::new(),
			text_width: 80,
			file_type,
			theme: tome_theme::get_theme(tome_theme::DEFAULT_THEME_ID)
				.unwrap_or(&tome_theme::DEFAULT_THEME),
			window_width: None,
			window_height: None,
			ui: {
				let mut ui = UiManager::new();
				ui.register_panel(Box::new(crate::ui::panels::terminal::TerminalPanel::new()));
				ui
			},
			needs_redraw: false,
			insert_undo_active: false,
			notifications: Notifications::new()
				.max_concurrent(Some(5))
				.overflow(Overflow::DiscardOldest),
			last_tick: std::time::SystemTime::now(),
			ipc: crate::ipc::IpcServer::start().ok(),
			completions: CompletionState::default(),
			extensions: {
				let mut map = ExtensionMap::new();
				let mut sorted_exts: Vec<_> = EXTENSIONS.iter().collect();
				sorted_exts.sort_by_key(|e| e.priority);
				for ext in sorted_exts {
					(ext.init)(&mut map);
				}
				map
			},
			fs,
			language_loader,
			syntax,
			style_overlays: StyleOverlays::new(),
		}
	}

	pub fn mode(&self) -> Mode {
		self.input.mode()
	}

	pub fn mode_name(&self) -> &'static str {
		self.input.mode_name()
	}

	pub fn ui_startup(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.startup();
		self.ui = ui;
		self.needs_redraw = true;
	}

	pub fn ui_tick(&mut self) {
		let mut ui = std::mem::take(&mut self.ui);
		ui.tick(self);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
	}

	pub fn tick(&mut self) {
		use crate::editor::extensions::TICK_EXTENSIONS;

		let mut sorted_ticks: Vec<_> = TICK_EXTENSIONS.iter().collect();
		sorted_ticks.sort_by_key(|e| e.priority);
		for ext in sorted_ticks {
			(ext.tick)(self);
		}
	}

	/// Updates style overlays based on current editor state.
	///
	/// This is called at the start of each render to ensure overlays reflect
	/// the current cursor position, even after mouse clicks or other events
	/// that modify state after the tick cycle.
	///
	/// Extensions that need to provide style overlays should register a
	/// RENDER_EXTENSIONS callback instead of (or in addition to) TICK_EXTENSIONS.
	pub fn update_style_overlays(&mut self) {
		use crate::editor::extensions::RENDER_EXTENSIONS;

		// Clear existing static overlays (keeps incomplete animations)
		self.style_overlays.clear();

		// Run render extensions to populate overlays
		let mut sorted: Vec<_> = RENDER_EXTENSIONS.iter().collect();
		sorted.sort_by_key(|e| e.priority);
		for ext in sorted {
			(ext.update)(self);
		}

		// Request redraw if there are active animations
		if self.style_overlays.has_animations() {
			self.needs_redraw = true;
		}
	}

	pub fn any_panel_open(&self) -> bool {
		self.ui.any_panel_open()
	}

	pub fn insert_text(&mut self, text: &str) {
		self.save_insert_undo_state();

		// Collapse all selections to their insertion points (line starts for ranges) so we insert at each cursor.
		let mut insertion_points = self.selection.clone();
		insertion_points.transform_mut(|r| {
			let pos = r.min();
			r.anchor = pos;
			r.head = pos;
		});

		let tx = Transaction::insert(self.doc.slice(..), &insertion_points, text.to_string());
		let mut new_selection = tx.map_selection(&insertion_points);
		new_selection.transform_mut(|r| {
			let pos = r.max();
			r.anchor = pos;
			r.head = pos;
		});
		self.apply_transaction(&tx);

		self.selection = new_selection;
		self.cursor = self.selection.primary().head;
	}

	pub fn yank_selection(&mut self) {
		let primary = self.selection.primary();
		let from = primary.min();
		let to = primary.max();
		if from < to {
			self.registers.yank = self.doc.slice(from..to).to_string();
			self.notify("info", format!("Yanked {} chars", to - from));
		}
	}

	pub fn paste_after(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		let slice = self.doc.slice(..);
		self.selection.transform_mut(|r| {
			*r = movement::move_horizontally(
				slice,
				*r,
				tome_base::range::Direction::Forward,
				1,
				false,
			);
		});
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn paste_before(&mut self) {
		if self.registers.yank.is_empty() {
			return;
		}
		self.insert_text(&self.registers.yank.clone());
	}

	pub fn handle_window_resize(&mut self, width: u16, height: u16) {
		self.window_width = Some(width);
		self.window_height = Some(height);
		self.text_width = width.saturating_sub(self.gutter_width()) as usize;
		let mut ui = std::mem::take(&mut self.ui);
		ui.notify_resize(self, width, height);
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;
		self.needs_redraw = true;
	}

	pub fn handle_focus_in(&mut self) {
		self.needs_redraw = true;
	}

	pub fn handle_focus_out(&mut self) {
		self.needs_redraw = true;
	}

	pub fn handle_paste(&mut self, content: String) {
		let mut ui = std::mem::take(&mut self.ui);
		let handled = ui.handle_paste(self, content.clone());
		if ui.take_wants_redraw() {
			self.needs_redraw = true;
		}
		self.ui = ui;

		if handled {
			self.needs_redraw = true;
			return;
		}

		self.insert_text(&content);
	}

	pub fn delete_selection(&mut self) {
		if !self.selection.primary().is_empty() {
			self.save_undo_state();
			let tx = Transaction::delete(self.doc.slice(..), &self.selection);
			self.selection = tx.map_selection(&self.selection);
			self.apply_transaction(&tx);
		}
	}

	pub fn set_theme(&mut self, theme_name: &str) -> Result<(), tome_manifest::CommandError> {
		if let Some(theme) = tome_theme::get_theme(theme_name) {
			self.theme = theme;
			Ok(())
		} else {
			let mut err = format!("Theme not found: {}", theme_name);
			if let Some(suggestion) = tome_theme::suggest_theme(theme_name) {
				err.push_str(&format!(". Did you mean '{}'?", suggestion));
			}
			Err(tome_manifest::CommandError::Failed(err))
		}
	}

	pub fn set_filesystem(&mut self, fs: Arc<dyn FileSystem>) {
		self.fs = fs;
	}

	/// Collects syntax highlight spans for the visible viewport.
	///
	/// Returns an empty vector if no syntax tree is available.
	pub fn collect_highlight_spans(
		&self,
		area: ratatui::layout::Rect,
	) -> Vec<(
		tome_language::highlight::HighlightSpan,
		ratatui::style::Style,
	)> {
		let Some(ref syntax) = self.syntax else {
			return Vec::new();
		};

		// Calculate byte range for visible viewport
		let start_line = self.scroll_line;
		let end_line = (start_line + area.height as usize).min(self.doc.len_lines());

		let start_byte = self.doc.line_to_byte(start_line) as u32;
		let end_byte = if end_line < self.doc.len_lines() {
			self.doc.line_to_byte(end_line) as u32
		} else {
			self.doc.len_bytes() as u32
		};

		// Create highlight styles from theme to resolve captures to abstract styles
		let highlight_styles =
			tome_language::highlight::HighlightStyles::new(SyntaxStyles::scope_names(), |scope| {
				self.theme.colors.syntax.resolve(scope)
			});

		// Get highlighter for visible range
		let highlighter = syntax.highlighter(
			self.doc.slice(..),
			&self.language_loader,
			start_byte..end_byte,
		);

		// Collect spans with resolved styles, converting at UI boundary
		highlighter
			.map(|span| {
				let abstract_style = highlight_styles.style_for_highlight(span.highlight);
				let ratatui_style: ratatui::style::Style = abstract_style.into();
				(span, ratatui_style)
			})
			.collect()
	}

	/// Gets the syntax highlighting style for a byte position.
	///
	/// Looks up the innermost highlight span containing this position.
	pub fn style_for_byte_pos(
		&self,
		byte_pos: usize,
		spans: &[(
			tome_language::highlight::HighlightSpan,
			ratatui::style::Style,
		)],
	) -> Option<ratatui::style::Style> {
		// Find the innermost span containing this position
		// Spans are ordered, later spans may be nested inside earlier ones
		for (span, style) in spans.iter().rev() {
			if byte_pos >= span.start as usize && byte_pos < span.end as usize {
				return Some(*style);
			}
		}
		None
	}

	/// Applies style overlays to a syntax style at a given byte position.
	///
	/// This modifies the style based on any active overlays (e.g., dimming
	/// from zen mode). Returns the modified style, or the original if no
	/// overlays apply.
	pub fn apply_style_overlay(
		&self,
		byte_pos: usize,
		style: Option<ratatui::style::Style>,
	) -> Option<ratatui::style::Style> {
		use crate::editor::extensions::StyleMod;
		use tome_theme::blend_colors;

		let Some(modification) = self.style_overlays.modification_at(byte_pos) else {
			return style;
		};

		let style = style.unwrap_or_default();
		let modified = match modification {
			StyleMod::Dim(factor) => {
				// Blend foreground color toward background
				let bg = self.theme.colors.ui.bg;
				if let Some(ratatui::style::Color::Rgb(r, g, b)) = style.fg {
					let fg = tome_base::color::Color::Rgb(r, g, b);
					let dimmed = blend_colors(fg, bg, factor);
					let tome_base::color::Color::Rgb(dr, dg, db) = dimmed else {
						return Some(style);
					};
					style.fg(ratatui::style::Color::Rgb(dr, dg, db))
				} else {
					// For non-RGB colors, apply a simple dimming by blending with gray
					style.fg(ratatui::style::Color::DarkGray)
				}
			}
			StyleMod::Fg(color) => {
				let ratatui_color: ratatui::style::Color = color.into();
				style.fg(ratatui_color)
			}
			StyleMod::Bg(color) => {
				let ratatui_color: ratatui::style::Color = color.into();
				style.bg(ratatui_color)
			}
		};

		Some(modified)
	}

	/// Applies a transaction to the document with incremental syntax tree update.
	///
	/// This is the central method for all document modifications. It:
	/// 1. Applies the changeset to the rope
	/// 2. Incrementally updates the syntax tree (if present)
	/// 3. Sets the modified flag
	///
	/// All edit operations should use this method to ensure the syntax tree stays in sync.
	pub fn apply_transaction(&mut self, tx: &Transaction) {
		// Capture old document state for incremental syntax update
		let old_doc = self.doc.clone();

		// Apply the transaction to the document
		tx.apply(&mut self.doc);

		// Incrementally update syntax tree if present.
		// Errors are silently ignored since syntax highlighting is non-critical
		// and eprintln! would corrupt the TUI. A future improvement would be to
		// fall back to a full reparse on error.
		if let Some(ref mut syntax) = self.syntax {
			let _ = syntax.update_from_changeset(
				old_doc.slice(..),
				self.doc.slice(..),
				tx.changes(),
				&self.language_loader,
			);
		}

		self.modified = true;
	}

	/// Reparses the entire syntax tree from scratch.
	///
	/// Used after operations that replace the entire document (undo/redo).
	pub fn reparse_syntax(&mut self) {
		if self.syntax.is_some() {
			// Get the language from the existing syntax tree
			let lang_id = self.syntax.as_ref().unwrap().root_language();
			self.syntax = Syntax::new(self.doc.slice(..), lang_id, &self.language_loader).ok();
		}
	}

	/// Converts a filesystem path to a virtual path for HostFS.
	///
	/// HostFS treats paths as relative to its root directory. This method:
	/// - For absolute paths: attempts to make them relative to cwd, or uses the
	///   full path as-is (HostFS will strip the leading /)
	/// - For relative paths: uses them directly
	/// - Returns None if the path contains non-UTF8 characters
	fn path_to_virtual(&self, path: &PathBuf) -> Option<String> {
		let cwd = std::env::current_dir().ok()?;
		Self::compute_virtual_path(path, &cwd)
	}
}
