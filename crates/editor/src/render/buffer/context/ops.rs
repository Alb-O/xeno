use ropey::Rope;
use xeno_primitives::{Mode, visible_line_count};
use xeno_registry::gutter::GutterAnnotations;
use xeno_runtime_language::LanguageId;
use xeno_runtime_language::highlight::HighlightSpan;
use xeno_tui::layout::Rect;
use xeno_tui::style::{Modifier, Style};

use super::super::diff::{DiffLineNumbers, compute_diff_line_numbers, diff_line_bg};
use super::super::gutter::GutterLayout;
use super::super::index::{HighlightIndex, OverlayIndex};
use super::super::plan::{LineSlice, LineSource, RowKind, ViewportPlan};
use super::super::row::{GutterRenderer, RowRenderInput, TextRowRenderer};
use super::super::style_layers::LineStyleContext;
use super::types::{
	BufferRenderContext, CursorStyles, RenderBufferParams, RenderLayout, RenderResult,
};
use crate::buffer::Buffer;
use crate::core::document::{Document, DocumentId};
use crate::render::cache::{HighlightSpanQuery, RenderCache};
use crate::render::wrap::WrappedSegment;
use crate::window::GutterSelector;

fn line_to_byte_or_eof(doc_content: &Rope, line: usize) -> u32 {
	if line < doc_content.len_lines() {
		doc_content.line_to_byte(line) as u32
	} else {
		doc_content.len_bytes() as u32
	}
}

impl<'a> BufferRenderContext<'a> {
	/// Creates cursor styling configuration based on theme and mode.
	pub fn make_cursor_styles(&self, mode: Mode) -> CursorStyles {
		let ui = &self.theme.colors.ui;
		let mode_color = self.mode_color(mode);

		let primary_cursor_style = Style::default()
			.bg(mode_color)
			.fg(ui.cursor_fg)
			.add_modifier(Modifier::BOLD);

		let secondary_cursor_style = {
			let bg = mode_color.blend(ui.bg, 0.4);
			let fg = ui.cursor_fg.blend(ui.fg, 0.4);
			Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD)
		};

		let base_style = Style::default().fg(ui.fg);

		let selection_style = Style::default().bg(ui.selection_bg).fg(ui.selection_fg);

		CursorStyles {
			primary: primary_cursor_style,
			secondary: secondary_cursor_style,
			base: base_style,
			selection: selection_style,
			unfocused: secondary_cursor_style,
		}
	}

	/// Returns the background color for the given mode's status badge.
	fn mode_color(&self, mode: Mode) -> xeno_tui::style::Color {
		self.theme.colors.mode.for_mode(&mode).bg
	}

	/// Collects styled highlight spans for the visible viewport of a document.
	///
	/// Returns an empty vec if no syntax tree is installed.
	///
	/// Uses the render cache to avoid recomputing highlights every frame.
	pub fn collect_highlight_spans(
		&self,
		doc_id: DocumentId,
		doc_content: &Rope,
		doc_version: u64,
		language_id: Option<LanguageId>,
		scroll_line: usize,
		viewport_height: usize,
		cache: &mut RenderCache,
	) -> Vec<(HighlightSpan, Style)> {
		let Some(syntax) = self.syntax_manager.syntax_for_doc(doc_id) else {
			return Vec::new();
		};
		let syntax_version = self.syntax_manager.syntax_version(doc_id);
		let projection = self
			.syntax_manager
			.highlight_projection_ctx(doc_id, doc_version);

		// Coverage check for partial trees
		if syntax.is_partial() {
			let total_lines = doc_content.len_lines();
			let start_line = scroll_line.min(total_lines);
			let end_line = start_line.saturating_add(viewport_height).min(total_lines);
			let viewport_start_byte = line_to_byte_or_eof(doc_content, start_line);
			let viewport_end_byte = line_to_byte_or_eof(doc_content, end_line);

			if let Some(coverage) = self.syntax_manager.syntax_coverage(doc_id)
				&& (viewport_end_byte <= coverage.start || viewport_start_byte >= coverage.end)
			{
				// No overlap with partial coverage. Skip highlights until viewport parsing catches up.
				return Vec::new();
			}
		}

		// Note: We allow rendering with a stale tree (tree_doc_version != doc_version)
		// to maintain highlighting during rapid edits. The highlight cache and
		// tile builder ensure safety via bounds clamping and version-keyed caching.

		let total_lines = visible_line_count(doc_content.slice(..));
		let start_line = scroll_line.min(total_lines);
		let end_line = start_line.saturating_add(viewport_height).min(total_lines);

		cache.highlight.get_spans(HighlightSpanQuery {
			doc_id,
			syntax_version,
			language_id,
			rope: doc_content,
			syntax,
			projection,
			language_loader: self.language_loader,
			style_resolver: |scope: &str| self.theme.colors.syntax.resolve(scope),
			start_line,
			end_line,
		})
	}

	/// Gets the diagnostic severity for a character position on a line.
	pub fn diagnostic_severity_at(&self, line_idx: usize, char_idx: usize) -> Option<u8> {
		let spans = self.diagnostic_ranges?.get(&line_idx)?;
		let mut max_severity = 0u8;
		for span in spans {
			if char_idx >= span.start_char && char_idx < span.end_char {
				max_severity = max_severity.max(span.severity);
			}
		}
		if max_severity > 0 {
			Some(max_severity)
		} else {
			None
		}
	}

	/// Applies diagnostic underline styling to a style if the position has a diagnostic.
	pub fn apply_diagnostic_underline(
		&self,
		line_idx: usize,
		char_idx: usize,
		style: Style,
	) -> Style {
		let Some(severity) = self.diagnostic_severity_at(line_idx, char_idx) else {
			return style;
		};

		use xeno_tui::style::UnderlineStyle;

		let underline_color = match severity {
			4 => self.theme.colors.semantic.error,
			3 => self.theme.colors.semantic.warning,
			2 => self.theme.colors.semantic.info,
			1 => self.theme.colors.semantic.hint,
			_ => return style,
		};

		style
			.underline_style(UnderlineStyle::Curl)
			.underline_color(underline_color)
	}

	/// Renders a buffer into a paragraph widget using registry gutters.
	pub fn render_buffer(
		&self,
		buffer: &Buffer,
		area: Rect,
		use_block_cursor: bool,
		is_focused: bool,
		tab_width: usize,
		cursorline: bool,
		cache: &mut RenderCache,
	) -> RenderResult {
		self.render_buffer_with_gutter(RenderBufferParams {
			buffer,
			area,
			use_block_cursor,
			is_focused,
			gutter: GutterSelector::Registry,
			tab_width,
			cursorline,
			cache,
		})
	}

	/// Renders a buffer into gutter and text columns.
	///
	/// Orchestrates the full rendering pipeline for a single buffer viewport.
	pub fn render_buffer_with_gutter(&self, p: RenderBufferParams<'_>) -> RenderResult {
		// Snapshot document state. This is the only place we touch the document lock.
		let (doc_id, doc_content, doc_version, total_lines, language_id, file_type, path) =
			p.buffer.with_doc(|doc: &Document| {
				let content = doc.content().clone();
				let total_lines = content.len_lines();
				(
					doc.id,
					content,
					doc.version(),
					total_lines,
					doc.language_id(),
					doc.file_type().map(String::from),
					doc.path().cloned(),
				)
			});

		let is_diff_file = file_type.as_deref().is_some_and(|ft| ft == "diff");

		let effective_gutter = if is_diff_file {
			Self::diff_gutter_selector(p.gutter)
		} else {
			p.gutter
		};

		let gutter_layout =
			GutterLayout::from_selector(effective_gutter, total_lines, p.area.width);
		let gutter_width = gutter_layout.total_width;
		let text_width = p.area.width.saturating_sub(gutter_width) as usize;
		let viewport_height = p.area.height as usize;

		let layout = RenderLayout {
			total_lines,
			gutter_layout,
			text_width,
		};

		let styles = self.make_cursor_styles(p.buffer.mode());
		let cursor_style_set = styles.to_cursor_set();
		let highlight_spans = self.collect_highlight_spans(
			doc_id,
			&doc_content,
			doc_version,
			language_id,
			p.buffer.scroll_line,
			viewport_height,
			p.cache,
		);
		let highlight_index = HighlightIndex::new(highlight_spans);

		let diff_line_numbers = if is_diff_file {
			Some(compute_diff_line_numbers(&doc_content))
		} else {
			None
		};

		let mode_color = self.mode_color(p.buffer.mode());
		let base_bg = self.theme.colors.ui.bg;

		// Use snapped doc_content for line calculations to avoid re-locking
		let cursor_line = doc_content.char_to_line(p.buffer.cursor.min(doc_content.len_chars()));

		let overlays = OverlayIndex::new(
			&p.buffer.selection,
			p.buffer.cursor,
			p.is_focused,
			&doc_content,
		);

		let start_line = p.buffer.scroll_line;
		let end_line = (start_line + viewport_height + 2).min(total_lines);
		let wrap_key = (text_width, p.tab_width);

		// Reverse order to avoid borrow conflict: build first, then get reference.
		p.cache.wrap.build_range(
			doc_id,
			wrap_key,
			&doc_content,
			doc_version,
			start_line,
			end_line,
		);
		let wrap_bucket = p.cache.wrap.get_or_build(doc_id, wrap_key);

		let plan = ViewportPlan::new_with_wrap(
			p.buffer.scroll_line,
			p.buffer.scroll_segment,
			viewport_height,
			total_lines,
			&*wrap_bucket,
		);

		let mut gutter_lines = Vec::with_capacity(viewport_height);
		let mut text_lines = Vec::with_capacity(viewport_height);

		for row in plan.rows {
			let (line, segment, is_continuation, is_last_segment) = match row.kind {
				RowKind::Text { line_idx, seg_idx } => {
					let slice: Option<LineSlice> = LineSource::load(&doc_content, line_idx);
					let segments: Option<&[WrappedSegment]> =
						wrap_bucket.get_segments(line_idx, doc_version);
					let num_segs = segments
						.map(|s: &[WrappedSegment]| s.len())
						.unwrap_or(0)
						.max(1);
					let segment = segments.and_then(|s: &[WrappedSegment]| s.get(seg_idx));
					(slice, segment, seg_idx > 0, seg_idx == num_segs - 1)
				}
				RowKind::NonTextBeyondEof => (None, None, false, true),
			};

			let line_idx = line
				.as_ref()
				.map(|l: &LineSlice| l.line_idx)
				.unwrap_or(total_lines);

			let diff_nums = diff_line_numbers
				.as_ref()
				.and_then(|nums: &Vec<DiffLineNumbers>| nums.get(line_idx));
			let line_annotations = GutterAnnotations {
				diagnostic_severity: self
					.diagnostics
					.and_then(|d| d.get(&line_idx).copied())
					.unwrap_or(0),
				sign: None,
				diff_old_line: diff_nums.and_then(|dn: &DiffLineNumbers| dn.old),
				diff_new_line: diff_nums.and_then(|dn: &DiffLineNumbers| dn.new),
			};

			let line_diff_bg = if is_diff_file {
				let line_text = line
					.as_ref()
					.map(|l: &LineSlice| l.content_string(&doc_content))
					.unwrap_or_default();
				diff_line_bg(true, &line_text, self.theme)
			} else {
				None
			};

			let line_style = LineStyleContext {
				base_bg: if matches!(row.kind, RowKind::NonTextBeyondEof) {
					self.theme.colors.ui.nontext_bg
				} else {
					base_bg
				},
				diff_bg: line_diff_bg,
				mode_color,
				is_cursor_line: p.cursorline && line_idx == cursor_line,
				cursorline_enabled: p.cursorline,
				cursor_line,
				is_nontext: matches!(row.kind, RowKind::NonTextBeyondEof),
			};

			let row_input = RowRenderInput {
				ctx: self,
				theme_cursor_styles: &styles,
				cursor_style_set,
				line_style,
				layout: &layout,
				buffer_path: path.as_deref(),
				is_focused: p.is_focused,
				use_block_cursor: p.use_block_cursor,
				tab_width: p.tab_width,
				doc_content: &doc_content,
				line: line.as_ref(),
				segment,
				is_continuation,
				is_last_segment,
				highlight: &highlight_index,
				overlays: &overlays,
				line_annotations,
			};

			gutter_lines.push(GutterRenderer::render_row(&row_input));
			text_lines.push(TextRowRenderer::render_row(&row_input));
		}

		RenderResult {
			gutter_width,
			gutter: gutter_lines,
			text: text_lines,
		}
	}

	/// Transforms a gutter selector for diff files by replacing standard line
	/// number gutters with `diff_line_numbers` while keeping other gutters intact.
	pub fn diff_gutter_selector(selector: GutterSelector) -> GutterSelector {
		static DIFF_WITH_SIGNS: &[&str] = &["diff_line_numbers", "signs"];
		static DIFF_ONLY: &[&str] = &["diff_line_numbers"];

		match selector {
			GutterSelector::Registry => GutterSelector::Named(DIFF_WITH_SIGNS),
			GutterSelector::Named(names) => {
				let has_line_nums = names.iter().any(|n| {
					matches!(
						*n,
						"line_numbers" | "relative_line_numbers" | "hybrid_line_numbers"
					)
				});
				let has_signs = names.contains(&"signs");

				match (has_line_nums, has_signs) {
					(true, true) => GutterSelector::Named(DIFF_WITH_SIGNS),
					(true, false) => GutterSelector::Named(DIFF_ONLY),
					(false, _) => GutterSelector::Named(names),
				}
			}
			other => other,
		}
	}
}
