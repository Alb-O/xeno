#[cfg(test)]
mod tests {
	use xeno_primitives::Rope;
	use xeno_primitives::range::CharIdx;
	use xeno_primitives::selection::Selection;
	use xeno_tui::style::{Color, Style};

	use crate::render::buffer::GutterLayout;
	use crate::render::buffer::context::types::{BufferRenderContext, CursorStyles, RenderLayout};
	use crate::render::buffer::index::{HighlightIndex, OverlayIndex};
	use crate::render::buffer::plan::LineSlice;
	use crate::render::buffer::row::{RowRenderInput, TextRowRenderer};
	use crate::render::buffer::style_layers::LineStyleContext;
	use crate::render::wrap::WrappedSegment;

	#[test]
	fn test_cursor_spans_tab_width() {
		let doc = Rope::from("\tX");
		let tab_width = 4;
		let theme = xeno_registry::themes::get_theme("default").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::new();

		let line_idx = 0;
		let line_slice = LineSlice {
			line_idx,
			start_char: 0,
			start_byte: 0,
			content_end_char: doc.len_chars(),
			end_char_incl_nl: doc.len_chars(),
			has_newline: false,
		};

		let segment = WrappedSegment {
			start_char_offset: 0,
			char_len: doc.len_chars(),
			indent_cols: 0,
		};

		let sel = Selection::point(0 as CharIdx);
		let overlays = OverlayIndex::new(&sel, 0 as CharIdx, true, &doc);
		let highlight = HighlightIndex::new(vec![]);

		let ctx = BufferRenderContext {
			theme: &theme,
			language_loader: &loader,
			diagnostics: None,
			diagnostic_ranges: None,
		};

		let cursor_styles = CursorStyles {
			primary: Style::default().bg(Color::Red),
			secondary: Style::default().bg(Color::Blue),
			unfocused: Style::default().bg(Color::Gray),
			selection: Style::default().bg(Color::Cyan),
			base: Style::default(),
		};

		let layout = RenderLayout {
			text_width: 20,
			total_lines: 1,
			gutter_layout: GutterLayout::hidden(),
		};

		let input = RowRenderInput {
			ctx: &ctx,
			theme_cursor_styles: &cursor_styles,
			cursor_style_set: cursor_styles.to_cursor_set(),
			line_style: LineStyleContext {
				base_bg: Color::Black,
				diff_bg: None,
				mode_color: Color::White,
				is_cursor_line: true,
				cursorline_enabled: false,
				cursor_line: 0,
				is_nontext: false,
			},
			layout: &layout,
			buffer_path: None,
			is_focused: true,
			use_block_cursor: false,
			tab_width,
			doc_content: &doc,
			line: Some(&line_slice),
			segment: Some(&segment),
			is_continuation: false,
			is_last_segment: true,
			highlight: &highlight,
			overlays: &overlays,
			line_annotations: Default::default(),
		};

		let line = TextRowRenderer::render_row(&input);
		let spans = line.spans;

		let tab_spans: Vec<_> = spans
			.iter()
			.take_while(|s| s.content.chars().all(|c| c == ' '))
			.collect();
		let total_tab_width: usize = tab_spans.iter().map(|s| s.content.len()).sum();
		assert_eq!(total_tab_width, tab_width);

		// Verify that the spans covering the tab width have the cursor background.
		// Since styles merge, we might get one span of 4 spaces with red bg.
		let cursor_width: usize = tab_spans
			.iter()
			.filter(|s| s.style.bg == Some(Color::Red))
			.map(|s| s.content.len())
			.sum();
		assert_eq!(
			cursor_width, tab_width,
			"The cursor background should span the full tab width"
		);
	}

	#[test]
	fn test_selection_spans_tab_width() {
		let doc = Rope::from("\tX");
		let tab_width = 4;
		let theme = xeno_registry::themes::get_theme("default").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::new();

		let line_idx = 0;
		let line_slice = LineSlice {
			line_idx,
			start_char: 0,
			start_byte: 0,
			content_end_char: doc.len_chars(),
			end_char_incl_nl: doc.len_chars(),
			has_newline: false,
		};

		let segment = WrappedSegment {
			start_char_offset: 0,
			char_len: doc.len_chars(),
			indent_cols: 0,
		};

		let sel = Selection::single(0 as CharIdx, 1 as CharIdx);
		let overlays = OverlayIndex::new(&sel, 1 as CharIdx, true, &doc);
		let highlight = HighlightIndex::new(vec![]);

		let ctx = BufferRenderContext {
			theme: &theme,
			language_loader: &loader,
			diagnostics: None,
			diagnostic_ranges: None,
		};

		let cursor_styles = CursorStyles {
			primary: Style::default().bg(Color::Red),
			secondary: Style::default().bg(Color::Blue),
			unfocused: Style::default().bg(Color::Gray),
			selection: Style::default().bg(Color::Cyan),
			base: Style::default(),
		};

		let layout = RenderLayout {
			text_width: 20,
			total_lines: 1,
			gutter_layout: GutterLayout::hidden(),
		};

		let input = RowRenderInput {
			ctx: &ctx,
			theme_cursor_styles: &cursor_styles,
			cursor_style_set: cursor_styles.to_cursor_set(),
			line_style: LineStyleContext {
				base_bg: Color::Black,
				diff_bg: None,
				mode_color: Color::White,
				is_cursor_line: true,
				cursorline_enabled: false,
				cursor_line: 0,
				is_nontext: false,
			},
			layout: &layout,
			buffer_path: None,
			is_focused: true,
			use_block_cursor: false,
			tab_width,
			doc_content: &doc,
			line: Some(&line_slice),
			segment: Some(&segment),
			is_continuation: false,
			is_last_segment: true,
			highlight: &highlight,
			overlays: &overlays,
			line_annotations: Default::default(),
		};

		let line = TextRowRenderer::render_row(&input);
		let spans = line.spans;

		let tab_spans: Vec<_> = spans
			.iter()
			.take_while(|s| s.content.chars().all(|c| c == ' '))
			.collect();
		let total_tab_width: usize = tab_spans.iter().map(|s| s.content.len()).sum();
		assert_eq!(total_tab_width, tab_width);

		for span in tab_spans {
			assert!(
				span.style.bg.is_some(),
				"Tab cell should have selection background"
			);
		}
	}

	#[test]
	fn test_cursor_does_not_span_continuation_indent() {
		let doc = Rope::from("Long line that wraps");
		let theme = xeno_registry::themes::get_theme("default").unwrap();
		let loader = xeno_runtime_language::LanguageLoader::new();

		let line_idx = 0;
		let line_slice = LineSlice {
			line_idx,
			start_char: 0,
			start_byte: 0,
			content_end_char: doc.len_chars(),
			end_char_incl_nl: doc.len_chars(),
			has_newline: false,
		};

		// Segment starting at char 10, with 4 columns of indent
		let segment = WrappedSegment {
			start_char_offset: 10,
			char_len: 10,
			indent_cols: 4,
		};

		// Cursor at the start of the line (char 0).
		// Even if the shaper uses line.start_char for Layout glyphs,
		// the renderer should not paint the cursor there.
		let sel = Selection::point(0 as CharIdx);
		let overlays = OverlayIndex::new(&sel, 0 as CharIdx, true, &doc);
		let highlight = HighlightIndex::new(vec![]);

		let ctx = BufferRenderContext {
			theme: &theme,
			language_loader: &loader,
			diagnostics: None,
			diagnostic_ranges: None,
		};

		let cursor_styles = CursorStyles {
			primary: Style::default().bg(Color::Red),
			secondary: Style::default().bg(Color::Blue),
			unfocused: Style::default().bg(Color::Gray),
			selection: Style::default().bg(Color::Cyan),
			base: Style::default(),
		};

		let layout = RenderLayout {
			text_width: 20,
			total_lines: 1,
			gutter_layout: GutterLayout::hidden(),
		};

		let input = RowRenderInput {
			ctx: &ctx,
			theme_cursor_styles: &cursor_styles,
			cursor_style_set: cursor_styles.to_cursor_set(),
			line_style: LineStyleContext {
				base_bg: Color::Black,
				diff_bg: None,
				mode_color: Color::White,
				is_cursor_line: true,
				cursorline_enabled: false,
				cursor_line: 0,
				is_nontext: false,
			},
			layout: &layout,
			buffer_path: None,
			is_focused: true,
			use_block_cursor: true,
			tab_width: 4,
			doc_content: &doc,
			line: Some(&line_slice),
			segment: Some(&segment),
			is_continuation: true,
			is_last_segment: true,
			highlight: &highlight,
			overlays: &overlays,
			line_annotations: Default::default(),
		};

		let line = TextRowRenderer::render_row(&input);
		let spans = line.spans;

		// Consume prefix of 4 spaces from the span list
		fn take_prefix<'a>(
			spans: &'a [xeno_tui::text::Span<'static>],
			mut n: usize,
		) -> Vec<(&'a xeno_tui::style::Style, String)> {
			let mut out = Vec::new();
			for sp in spans {
				if n == 0 {
					break;
				}
				let content_len = sp.content.chars().count();
				let take = n.min(content_len);
				let chunk: String = sp.content.chars().take(take).collect();
				out.push((&sp.style, chunk));
				n -= take;
			}
			out
		}

		let prefix = take_prefix(&spans, 4);

		// 1) The first 4 rendered cells are spaces.
		let prefix_text: String = prefix.iter().map(|(_, s)| s.as_str()).collect();
		assert_eq!(&prefix_text, "    ");

		// 2) None of those prefix cells have cursor bg.
		for (style, chunk) in prefix {
			if !chunk.is_empty() {
				assert_ne!(
					style.bg,
					Some(Color::Red),
					"Indent cell should not have cursor background"
				);
			}
		}
	}
}
